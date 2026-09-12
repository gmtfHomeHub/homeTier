use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;

pub struct AuthState {
    pub secret: String,
}

pub fn generate_auth_secret() -> String {
    hex::encode(rand::thread_rng().gen::<[u8; 32]>())
}

pub fn init_auth_secret(config: &AppConfig) -> AuthState {
    let existing = config.get_str("SERVER_AUTH_SECRET", "");
    if !existing.is_empty() {
        return AuthState { secret: existing };
    }
    let new_secret = generate_auth_secret();
    let _ = config.set("SERVER_AUTH_SECRET", &new_secret);
    AuthState { secret: new_secret }
}

#[derive(Serialize, Deserialize)]
pub struct Fingerprint {
    pub host: String,
    pub user_agent: String,
    pub referer: String,
}

pub fn extract_fingerprint(headers: &axum::http::HeaderMap) -> String {
    let host = headers
        .get("Host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ua = headers
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let refr = headers
        .get("Referer")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(format!("{}|{}|{}", host, ua, refr).as_bytes()))
}

pub fn verify_request(headers: &axum::http::HeaderMap, secret: &str) -> bool {
    let cookie_val = headers
        .get("Cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|c| {
            c.split(';')
                .find(|s| {
                    let s = s.trim();
                    s.starts_with("__Host-token=") || s.starts_with("hometier-token=")
                })
                .map(|s| {
                    let s = s.trim();
                    if s.starts_with("__Host-token=") {
                        s["__Host-token=".len()..].to_string()
                    } else {
                        s["hometier-token=".len()..].to_string()
                    }
                })
        });

    match cookie_val {
        Some(token) => verify_token(&token, &extract_fingerprint(headers), secret),
        None => false,
    }
}

pub fn verify_token(token: &str, fingerprint: &str, secret: &str) -> bool {
    let raw = match URL_SAFE_NO_PAD.decode(token) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if raw.len() < 12 {
        return false;
    }
    let sig_bytes = &raw[12..];
    let expected_hex = crate::crypto::hmac_sha256(secret, fingerprint.as_bytes());
    let expected = hex::decode(expected_hex).unwrap_or_default();
    sig_bytes == expected.as_slice()
}

pub fn generate_cookie_value(fingerprint: &str, secret: &str) -> String {
    let nonce: [u8; 12] = rand::thread_rng().gen();
    let sig_hex = crate::crypto::hmac_sha256(secret, fingerprint.as_bytes());
    let sig_bytes = hex::decode(sig_hex).unwrap_or_default();
    let combined: Vec<u8> = nonce.iter().copied().chain(sig_bytes).collect();
    URL_SAFE_NO_PAD.encode(&combined)
}

/// 从请求路径中剥离公共前缀（如 /hometier），并确保结果以 / 开头
pub fn strip_public_base<'a>(path: &'a str, public_base: &str) -> &'a str {
    if public_base.is_empty() || public_base == "/" {
        return path;
    }
    // public_base 形如 /xxx 或 /xxx/，归一化去尾斜杠后比较
    let base = public_base.trim_end_matches('/');
    if path == base {
        return "/";
    }
    if let Some(rest) = path.strip_prefix(base) {
        if rest.is_empty() {
            "/"
        } else if rest.starts_with('/') {
            rest
        } else {
            path
        }
    } else {
        path
    }
}

pub fn is_static_resource(path: &str, public_base: &str) -> bool {
    let path = strip_public_base(path, public_base);
    let extensions = [
        ".html", ".css", ".js", ".ico", ".svg", ".png", ".jpg", ".jpeg",
        ".gif", ".woff", ".woff2", ".ttf", ".eot", ".map", ".txt", ".xml",
    ];
    let special_prefixes = ["/assets/", "/favicon.", "/robots.", "/manifest."];
    if path == "/" || path.is_empty() {
        return true;
    }
    for prefix in &special_prefixes {
        if path.starts_with(prefix) {
            return true;
        }
    }
    for ext in &extensions {
        if path.ends_with(ext) {
            return true;
        }
    }
    // SPA 前端路由（如 /space/:id、/settings）无文件扩展名，
    // 放行给 static_file_handler 返回 index.html（客户端路由渲染）。
    // 但 /api/ 与 /ws/ 路径必须校验 cookie。
    if !path.contains('.') && !path.starts_with("/api/") && !path.starts_with("/ws/") {
        return true;
    }
    false
}

pub fn set_cookie_header(
    response: &mut axum::response::Response,
    cookie_value: &str,
    secure: bool,
) {
    let cookie = if secure {
        format!(
            "__Host-token={}; Path=/; HttpOnly; SameSite=Lax; Secure",
            cookie_value
        )
    } else {
        format!(
            "hometier-token={}; Path=/; HttpOnly; SameSite=Lax",
            cookie_value
        )
    };
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&cookie).unwrap(),
    );
}