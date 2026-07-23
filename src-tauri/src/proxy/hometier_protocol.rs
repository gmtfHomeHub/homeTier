use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use http::Uri;
use reqwest::blocking::Client;
use sha2::Digest;
use sha2::Sha256;
use tauri::{AppHandle, Runtime, UriSchemeContext, UriSchemeResponder};

/// 代理服务器端口，由 lib.rs 在启动后设置
static PROXY_PORT: OnceLock<u16> = OnceLock::new();

pub fn set_proxy_port(port: u16) {
    let _ = PROXY_PORT.set(port);
}

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

fn strip_iframe_restrictions(resp: &mut http::Response<Vec<u8>>, script_hash: &str) {
    let headers = resp.headers_mut();
    headers.remove("x-frame-options");

    if script_hash.is_empty() {
        return;
    }

    // 处理 CSP：移除 frame-ancestors，添加 hometierproxy: 到 connect-src / default-src
    // 添加 script-src hash 放行注入脚本
    if let Some(csp) = headers.get("content-security-policy") {
        if let Ok(val) = csp.to_str() {
            let mut directives: Vec<String> = val
                .split(';')
                .map(|s| s.trim().to_string())
                .collect();
            let mut modified = false;
            let mut has_script_src = false;

            for directive in directives.iter_mut() {
                let trimmed = directive.trim().to_string();
                // 移除 frame-ancestors
                if trimmed.starts_with("frame-ancestors") {
                    directive.clear();
                    modified = true;
                }
                // 在 script-src 中添加 hash
                if trimmed.starts_with("script-src") {
                    has_script_src = true;
                    if !trimmed.contains(script_hash) {
                        *directive = format!("{} {}", trimmed, script_hash);
                        modified = true;
                    }
                }
                // 在 connect-src 中添加 hometierproxy:
                if trimmed.starts_with("connect-src") && !trimmed.contains("hometierproxy:") {
                    *directive = format!("{} hometierproxy:", trimmed);
                    modified = true;
                }
                // 在 default-src 中添加 hometierproxy:
                if trimmed.starts_with("default-src") && !trimmed.contains("hometierproxy:") {
                    *directive = format!("{} hometierproxy:", trimmed);
                    modified = true;
                }
            }

            // 如果 CSP 存在但没有 script-src，添加一个（放行注入脚本）
            if !has_script_src {
                directives.push(format!("script-src {}", script_hash));
                modified = true;
            }

            if modified {
                let joined: Vec<&str> = directives
                    .iter()
                    .map(|s| s.as_str())
                    .filter(|s| !s.is_empty())
                    .collect();
                let joined = joined.join("; ");
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
}

fn rewrite_html_body(body: &[u8], origin_str: &str) -> Vec<u8> {
    let old_prefix = format!("http://{}", origin_str);
    let new_prefix = format!("hometierproxy://{}", origin_str);
    match String::from_utf8(body.to_vec()) {
        Ok(html) => html.replace(&old_prefix, &new_prefix).into_bytes(),
        Err(_) => Vec::new(),
    }
}

/// 注入代理修复脚本：修复 location 属性 + 拦截 fetch/XHR 重写 URL
fn inject_proxy_script(html_bytes: Vec<u8>, host_key: &str) -> (Vec<u8>, String) {
    let mut html = match String::from_utf8(html_bytes.clone()) {
        Ok(h) => h,
        Err(_) => return (html_bytes, String::new()),
    };

    let proxy_port = PROXY_PORT.get().copied().unwrap_or(1420);
    let js_content = format!(
        r#"(function(){{
var H="{}",P="{}";
var _f=window.fetch;window.fetch=function(u,i){{if(typeof u=="string"){{u=r(u)}}else if(u&&u.url){{var nu=r(u.url);if(nu!==u.url)u=new Request(nu,u)}}return _f.call(this,u,i)}};
var _o=XMLHttpRequest.prototype.open;XMLHttpRequest.prototype.open=function(m,u){{if(typeof u=="string"){{arguments[1]=r(u)}}return _o.apply(this,arguments)}};
var _WS=window.WebSocket;window.WebSocket=function(u,p){{if(typeof u=="string"){{u=r_ws(u)}}return new _WS(u,p)}};window.WebSocket.prototype=_WS.prototype;window.WebSocket.CONNECTING=0;window.WebSocket.OPEN=1;window.WebSocket.CLOSING=2;window.WebSocket.CLOSED=3;
function r_ws(u){{var m=u.match(/^(wss?):\/\/(?:hometierproxy|127\.0\.0\.1|localhost)(?::\d+)?(?=\/|\?|#|$)/i);if(m)return u;var s=u.indexOf("://");if(s<0)return u;var sc=u.substring(0,s);var rest=u.substring(s+3);var p=rest.indexOf("/");var h=p>=0?rest.substring(0,p):rest;var pa=p>=0?rest.substring(p):"/";return "ws://127.0.0.1:"+P+"/"+(sc==="wss"?"wss":"ws")+"/"+h+pa}};
function r(u){{if(u.indexOf("hometierproxy://")===0)return u;if(u.charAt(0)==='/')return "hometierproxy://"+H+"/"+u.replace(/^\/+/,"");var m=u.match(/^https?:\/\/hometierproxy(?::\d+)?(?=\/|\?|#|$)/i);if(m)return u.replace(/^https?:\/\/[^\/]+/,"hometierproxy://"+H);return u.replace(RegExp("^https?://"+H.replace(/\./g,"\\.")+"(?=/|\\?|#|$)","i"),"hometierproxy://"+H)}};
}}})()"#,
        host_key, proxy_port
    );

    let hash = Sha256::digest(js_content.as_bytes());
    let encoded = base64::engine::general_purpose::STANDARD.encode(hash);
    let csp_hash = format!("'sha256-{}'", encoded);
    let script_tag = format!("<script id=\"__ht\">{}</script>", js_content);

    // 注入到 <head> 之后（最早执行，拦截页面脚本）
    let lower = html.to_lowercase();
    if let Some(pos) = lower.find("<head") {
        let after = pos + 5;
        let rest = &lower[after..];
        if let Some(close) = rest.find('>') {
            let inject_at = after + close + 1;
            html.insert_str(inject_at, &script_tag);
            return (html.into_bytes(), csp_hash);
        }
    }
    // fallback: 在 </head> 前注入
    if let Some(pos) = lower.find("</head>") {
        html.insert_str(pos, &script_tag);
    } else if let Some(pos) = lower.find("<body") {
        html.insert_str(pos, &script_tag);
    } else {
        html.insert_str(0, &script_tag);
    }
    (html.into_bytes(), csp_hash)
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
        http::Method::POST => client.post(&forward_url).body(body.clone()),
        http::Method::PUT => client.put(&forward_url).body(body.clone()),
        http::Method::PATCH => client.patch(&forward_url).body(body.clone()),
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
        if !hop_by_hop.contains(&key_lower.as_str()) && key_lower != "cookie" {
            if let Ok(val_str) = value.to_str() {
                req_builder = req_builder.header(key.as_str(), val_str);
            }
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

    let (body, script_hash) = if is_html_content(content_type) {
        let rewritten = rewrite_html_body(&body_bytes, &host_key);
        let (injected, hash) = inject_proxy_script(rewritten, &host_key);
        (injected, hash)
    } else {
        (body_bytes, String::new())
    };

    let mut response = builder
        .header("content-length", body.len().to_string())
        .header("access-control-allow-origin", "*")
        .body(body)
        .map_err(|e| format!("Failed to build response: {}", e))?;

    strip_iframe_restrictions(&mut response, &script_hash);

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
                            .header("access-control-allow-origin", "*")
                            .body(e.into_bytes())
                            .unwrap();
                        responder.respond(error_response);
                    }
                }
            });
        },
    )
}
