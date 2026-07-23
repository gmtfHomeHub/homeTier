use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use http::Uri;
use reqwest::blocking::Client;
use tauri::{AppHandle, Runtime, UriSchemeContext, UriSchemeResponder};

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
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(15))
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
                headers.insert("content-security-policy", joined.parse().unwrap());
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

// --- Cookie Jar ---

struct StoredCookie {
    name: String,
    value: String,
    expires_at: Option<u64>,
}

struct PerHostCookieJar(Vec<StoredCookie>);

impl PerHostCookieJar {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn add_set_cookie(&mut self, header: &str) {
        let parts: Vec<&str> = header.split(';').collect();
        if parts.is_empty() {
            return;
        }

        let first_eq = match parts[0].find('=') {
            Some(pos) => pos,
            None => return,
        };
        let name = parts[0][..first_eq].trim().to_string();
        let value = parts[0][first_eq + 1..].trim().to_string();

        let mut expires_at: Option<u64> = None;
        for part in &parts[1..] {
            let part = part.trim();
            if let Some(eq) = part.find('=') {
                let key = part[..eq].trim().to_lowercase();
                let val = part[eq + 1..].trim();
                if key == "max-age" {
                    if let Ok(secs) = val.parse::<i64>() {
                        if secs <= 0 {
                            expires_at = Some(0);
                        } else {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            expires_at = Some(now + secs as u64);
                        }
                    }
                }
            }
        }

        self.0.retain(|c| c.name != name);
        self.0.push(StoredCookie { name, value, expires_at });
    }

    fn build_cookie_header(&mut self) -> Option<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.0.retain(|c| match c.expires_at {
            Some(exp) => exp > now,
            None => true,
        });

        if self.0.is_empty() {
            return None;
        }

        Some(
            self.0
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }
}

// --- Request handling ---

fn handle_request<R: Runtime>(
    app_handle: &AppHandle<R>,
    clients: &Arc<Mutex<HashMap<String, Client>>>,
    cookie_jars: &Arc<Mutex<HashMap<String, PerHostCookieJar>>>,
    request: &http::Request<Vec<u8>>,
) -> Result<http::Response<Vec<u8>>, String> {
    let target = parse_target(request.uri())?;
    let forward_url = build_forward_url(&target);
    let host_key = build_origin_str(&target.host, target.port);
    let client = get_or_create_client(clients, &target.host);
    let method = request.method().clone();
    let req_headers = request.headers().clone();
    let body = request.body().clone();

    // 1. Build forwarded request with cookie injection
    let cookie_header = {
        let mut jars = cookie_jars.lock().unwrap();
        let jar = jars.entry(host_key.clone()).or_insert_with(PerHostCookieJar::new);
        jar.build_cookie_header()
    };

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

    // Forward browser headers, replacing cookie with our jar value
    for (key, value) in &req_headers {
        let key_lower = key.as_str().to_lowercase();
        if !hop_by_hop.contains(&key_lower) && key_lower != "cookie" {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }
    }
    if let Some(ref cookies) = cookie_header {
        req_builder = req_builder.header("Cookie", cookies.as_str());
    }

    // 2. Send upstream
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

    // 3. Capture Set-Cookie into jar
    for value in upstream_headers.get_all("set-cookie") {
        if let Ok(val) = value.to_str() {
            let mut jars = cookie_jars.lock().unwrap();
            let jar = jars.entry(host_key.clone()).or_insert_with(PerHostCookieJar::new);
            jar.add_set_cookie(val);
        }
    }

    // 4. Build proxy response (strip Set-Cookie from downstream)
    let mut builder = http::Response::builder().status(status);

    for (key, value) in &upstream_headers {
        let key_lower = key.as_str().to_lowercase();
        if key_lower == "content-length"
            || key_lower == "transfer-encoding"
            || key_lower == "content-encoding"
            || key_lower == "set-cookie"
        {
            continue;
        }
        builder = builder.header(key.as_str(), value.clone());
    }

    let body = if is_html_content(content_type) {
        rewrite_html_body(body_bytes, &host_key)
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
    let cookie_jars: Arc<Mutex<HashMap<String, PerHostCookieJar>>> =
        Arc::new(Mutex::new(HashMap::new()));

    builder.register_asynchronous_uri_scheme_protocol(
        "hometierproxy",
        move |ctx: UriSchemeContext<'_, R>,
              request: http::Request<Vec<u8>>,
              responder: UriSchemeResponder| {
            let app_handle = ctx.app_handle().clone();
            let clients = clients.clone();
            let cookie_jars = cookie_jars.clone();

            std::thread::spawn(move || {
                let result = handle_request(&app_handle, &clients, &cookie_jars, &request);
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
