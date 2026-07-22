use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use http::Uri;
use reqwest::blocking::Client;
use tauri::{AppHandle, Runtime, UriSchemeContext, UriSchemeResponder};

/// 协议 URL 中标识当前请求的来源，格式为 `hometierproxy://{host}:{port}/{path}`
struct ForwardTarget {
    host: String,
    port: Option<u16>,
    path_and_query: String,
}

fn parse_target(uri: &Uri) -> Result<ForwardTarget, String> {
    let host = uri
        .host()
        .ok_or_else(|| format!("Missing host in URI: {}", uri))?;
    let port = uri.port_u16();
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    Ok(ForwardTarget {
        host: host.to_string(),
        port,
        path_and_query,
    })
}

fn build_origin_str(host: &str, port: Option<u16>) -> String {
    match port {
        Some(p) => format!("{}:{}", host, p),
        None => host.to_string(),
    }
}

fn build_forward_url(target: &ForwardTarget) -> String {
    match target.port {
        Some(p) => format!(
            "http://{}:{}{}",
            target.host, p, target.path_and_query
        ),
        None => format!("http://{}{}", target.host, target.path_and_query),
    }
}

fn get_or_create_client(
    clients: &Arc<Mutex<HashMap<String, Client>>>,
    host: &str,
) -> Client {
    let mut map = clients.lock().unwrap();
    if let Some(client) = map.get(host) {
        return client.clone();
    }
    let client = Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    map.insert(host.to_string(), client.clone());
    client
}

fn strip_iframe_restrictions(resp: &mut http::Response<Vec<u8>>) {
    let headers = resp.headers_mut();
    headers.remove("x-frame-options");
    if let Some(csp) = headers.get("content-security-policy") {
        if let Ok(val) = csp.to_str() {
            let filtered: Vec<&str> = val
                .split(';')
                .map(|s| s.trim())
                .filter(|s| !s.starts_with("frame-ancestors"))
                .collect();
            let joined = filtered.join("; ");
            if joined.is_empty() {
                headers.remove("content-security-policy");
            } else {
                headers.insert(
                    "content-security-policy",
                    joined.parse().unwrap(),
                );
            }
        }
    }
}

fn rewrite_html_body(body: Vec<u8>, origin_str: &str) -> Vec<u8> {
    let old_prefix = format!("http://{}", origin_str);
    let new_prefix = format!("hometierproxy://{}", origin_str);
    match String::from_utf8(body) {
        Ok(html) => html.replace(&old_prefix, &new_prefix).into_bytes(),
        Err(_) => body,
    }
}

fn is_html_content(content_type: Option<&http::HeaderValue>) -> bool {
    match content_type.and_then(|v| v.to_str().ok()) {
        Some(ct) => {
            let ct = ct.to_lowercase();
            ct.contains("text/html") || ct.contains("application/xhtml")
        }
        None => false,
    }
}

fn handle_request<R: Runtime>(
    app_handle: &AppHandle<R>,
    clients: &Arc<Mutex<HashMap<String, Client>>>,
    request: &http::Request<Vec<u8>>,
) -> Result<http::Response<Vec<u8>>, String> {
    let target = parse_target(request.uri())?;
    let forward_url = build_forward_url(&target);
    let origin_str = build_origin_str(&target.host, target.port);
    let client = get_or_create_client(clients, &target.host);
    let method = request.method().clone();
    let headers = request.headers().clone();
    let body = request.body().clone();

    let mut req_builder = match method {
        http::Method::GET => client.get(&forward_url),
        http::Method::POST => client.post(&forward_url).body(body),
        http::Method::PUT => client.put(&forward_url).body(body),
        http::Method::PATCH => client.patch(&forward_url).body(body),
        http::Method::DELETE => client.delete(&forward_url),
        http::Method::HEAD => client.head(&forward_url),
        _ => client.get(&forward_url),
    };

    let hop_by_hop = [
        "host", "connection", "keep-alive", "proxy-authenticate",
        "proxy-authorization", "te", "trailers", "transfer-encoding", "upgrade",
    ];
    for (key, value) in &headers {
        let key_lower = key.as_str().to_lowercase();
        if !hop_by_hop.contains(&key_lower) {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }
    }

    let upstream = req_builder
        .send()
        .map_err(|e| format!("Forward request failed: {}", e))?;

    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let content_type = upstream_headers.get("content-type");
    let body_bytes = upstream
        .bytes()
        .map_err(|e| format!("Failed to read upstream body: {}", e))?
        .to_vec();

    let mut builder = http::Response::builder().status(status);

    for (key, value) in &upstream_headers {
        let key_lower = key.as_str().to_lowercase();
        if key_lower == "content-length"
            || key_lower == "transfer-encoding"
            || key_lower == "content-encoding"
        {
            continue;
        }
        builder = builder.header(key.as_str(), value.clone());
    }

    let body = if is_html_content(content_type) {
        rewrite_html_body(body_bytes, &origin_str)
    } else {
        body_bytes
    };

    let mut response = builder
        .header("content-length", body.len().to_string())
        .header("access-control-allow-origin", "*")
        .body(body)
        .map_err(|e| format!("Failed to build response: {}", e))?;

    strip_iframe_restrictions(&mut response);

    Ok(response)
}

pub fn register_protocol<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    let clients: Arc<Mutex<HashMap<String, Client>>> =
        Arc::new(Mutex::new(HashMap::new()));

    builder.register_asynchronous_uri_scheme_protocol(
        "hometierproxy",
        move |ctx: UriSchemeContext<'_, R>,
              request: http::Request<Vec<u8>>,
              responder: UriSchemeResponder| {
            let app_handle = ctx.app_handle().clone();
            let clients = clients.clone();

            std::thread::spawn(move || {
                let result = handle_request(&app_handle, &clients, &request);
                match result {
                    Ok(response) => responder.respond(response),
                    Err(e) => {
                        let error_response = http::Response::builder()
                            .status(500)
                            .header("content-type", "text/plain; charset=utf-8")
                            .body(e.into_bytes())
                            .unwrap();
                        responder.respond(error_response);
                    }
                }
            });
        },
    )
}
