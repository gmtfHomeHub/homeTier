use std::fmt::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::proxy::hometier_protocol;  // 用于共享 CookieJar

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

enum WsUpstream {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::TlsStream<TcpStream>>)
}

impl AsyncRead for WsUpstream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            WsUpstream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            WsUpstream::Tls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for WsUpstream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            WsUpstream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            WsUpstream::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            WsUpstream::Plain(s) => Pin::new(s).poll_flush(cx),
            WsUpstream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            WsUpstream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            WsUpstream::Tls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

fn extract_header<'a>(headers: &'a str, name: &str) -> &'a str {
    for line in headers.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().to_lowercase() == name {
                return v.trim();
            }
        }
    }
    ""
}

pub fn is_raw_ws_upgrade(data: &[u8]) -> bool {
    let s = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if !s.starts_with("GET") && !s.starts_with("get") {
        return false;
    }
    let lower = s.to_lowercase();
    lower.contains("upgrade:") && lower.contains("websocket")
}

/// 判断 hyper Request 是否为 WebSocket upgrade（用于 server.rs 中 serve_http 的回退检测）
pub fn is_ws_upgrade(req: &hyper::Request<hyper::body::Incoming>) -> bool {
    req.method() == hyper::Method::GET
        && req.headers().get("upgrade")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_lowercase().contains("websocket"))
            .unwrap_or(false)
        && req.headers().get("connection")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_lowercase().contains("upgrade"))
            .unwrap_or(false)
}

async fn connect_upstream(
    target_host: &str,
    target_port: u16,
    scheme: &str,
) -> Result<WsUpstream, String> {
    let addr = format!("{}:{}", target_host, target_port);
    let bare = match timeout(Duration::from_secs(10), TcpStream::connect(&addr)).await {
        Ok(Ok(s)) => s,
        _ => {
            let fallback = format!("127.0.0.1:{}", target_port);
            timeout(Duration::from_secs(10), TcpStream::connect(&fallback))
                .await
                .map_err(|_| format!("connect timeout: {} (fallback {})", addr, fallback))?
                .map_err(|e| {
                    format!(
                        "connect failed: {} and fallback {}: {}",
                        addr, fallback, e
                    )
                })?
        }
    };

    if scheme == "wss" {
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore::from_iter(
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
            ))
            .with_no_client_auth();
        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
        let domain = rustls::pki_types::ServerName::try_from(target_host.to_string())
            .map_err(|_| format!("invalid hostname: {}", target_host))?;
        let tls_stream = connector
            .connect(domain, bare)
            .await
            .map_err(|e| format!("tls connect: {}", e))?;
        Ok(WsUpstream::Tls(Box::new(tokio_rustls::TlsStream::Client(tls_stream))))
    } else {
        Ok(WsUpstream::Plain(bare))
    }
}

/// 直接从 TcpStream 处理 WebSocket 升级请求（无需 hyper upgrade 机制）。
/// 接收原始 TCP 流和已预读的请求数据，解析 HTTP 请求，连接上游，发送 101 响应，然后双向转发数据。
pub async fn handle_raw_upgrade(mut client: TcpStream, initial_data: Vec<u8>) {
    // 1. 使用外部传入的初始数据解析 HTTP 请求（数据由 server.rs 预读传入）
    let data = &initial_data[..];

    // 定位 \r\n\r\n（请求头结束）
    let eoh = match data.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(p) => p,
        None => return,
    };
    let header_str = match std::str::from_utf8(&data[..eoh]) {
        Ok(s) => s,
        Err(_) => return,
    };

    // 2. 解析请求行
    let request_line = match header_str.lines().next() {
        Some(l) => l,
        None => return,
    };
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let path = parts[1];

    // 3. 从查询参数解析 scheme 和目标地址 (格式: ?ws=encoded_target 或 ?wss=encoded_target)
    let (scheme, encoded_target) = if let Some(qs) = path.split('?').nth(1) {
        if let Some(val) = qs.strip_prefix("ws=") {
            ("ws", val)
        } else if let Some(val) = qs.strip_prefix("wss=") {
            ("wss", val)
        } else {
            crate::log_error!(format!("WS 代理: 无法解析查询参数: {}", path));
            return;
        }
    } else {
        crate::log_error!(format!("WS 代理: 缺少查询参数: {}", path));
        return;
    };

    // URL 解码目标地址
    let target_str = match urlencoding::decode(encoded_target) {
        Ok(s) => s.into_owned(),
        Err(_) => {
            crate::log_error!(format!("WS 代理: URL 解码失败: {}", encoded_target));
            return;
        }
    };

    // 解析目标地址: host:port/path?query
    let (authority, tpath) = match target_str.find('/') {
        Some(pos) => (&target_str[..pos], &target_str[pos..]),
        None => (&target_str[..], "/"),
    };
    let default_port: u16 = if scheme == "wss" { 443 } else { 80 };
    let (mut target_host, mut target_port) = match authority.rfind(':') {
        Some(pos) => (
            authority[..pos].to_string(),
            authority[pos + 1..].parse().unwrap_or(default_port),
        ),
        None => (authority.to_string(), default_port),
    };

    // 若路径中未解析出合法 host:port，尝试从 extra_headers 的 Origin 头中提取
    let target_host_port_valid = target_host.contains('.') || target_host.parse::<std::net::IpAddr>().is_ok();

    // 4. 提取 WS 专用头 + 收集所有非 hop-by-hop 请求头
    let hop_by_hop = [
        "host", "connection", "keep-alive", "proxy-authenticate",
        "proxy-authorization", "te", "trailers", "transfer-encoding", "upgrade",
    ];
    let mut ws_key = String::new();
    let mut ws_version = String::new();
    let mut extra_headers: Vec<(String, String)> = Vec::new();
    let mut client_cookie: Option<String> = None;

    for line in header_str.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let key_lower = key.to_lowercase();
            let value = v.trim().to_string();

            // crate::log_debug!(format!("WS 代理: 解析请求头 {}: {}", key_lower, value));

            match key_lower.as_str() {
                "sec-websocket-key" => ws_key = value,
                "sec-websocket-version" => ws_version = value,
                "cookie" => client_cookie = Some(value),
                _ => {
                    // 过滤 hop-by-hop 头，其余加入 extra_headers
                    if !hop_by_hop.contains(&key_lower.as_str()) {
                        extra_headers.push((key, value));
                    }
                }
            }
        }
    }

    // 若路径中未解析出合法 host:port（如 synoscgi.sock），从 Origin 头中提取
    if !target_host_port_valid {
        for (k, v) in &extra_headers {
            if k.to_lowercase() == "origin" {
                let origin_str = v.strip_prefix("http://").or_else(|| v.strip_prefix("https://")).unwrap_or("");
                if let Some(colon) = origin_str.rfind(':') {
                    let host = &origin_str[..colon];
                    if let Ok(port) = origin_str[colon + 1..].parse::<u16>() {
                        crate::log_info!(format!("WS 代理: 从 Origin 头提取 target={}:{}", host, port));
                        target_host = host.to_string();
                        target_port = port;
                    }
                }
                break;
            }
        }
    }

    crate::log_info!(format!("WS 代理请求: {}://{}:{}{}", scheme, target_host, target_port, tpath));

    // 5. 连接上游
    crate::log_info!(format!("WS 代理: 开始连接上游 {}:{} ({})", target_host, target_port, scheme));
    let mut upstream = match connect_upstream(&target_host, target_port, scheme).await {
        Ok(u) => {
            crate::log_info!(format!("WS 代理: 上游连接成功 {}:{}", target_host, target_port));
            u
        }
        Err(e) => {
            crate::log_error!(format!("WS 代理: 上游连接失败 {}:{} - {}", target_host, target_port, e));
            return;
        }
    };

    // 6. 发送 WS upgrade 请求到上游（包含所有转发头 + cookie）
    let origin_val = if scheme == "wss" { "https" } else { "http" };
    let host_header = format!("{}:{}", target_host, target_port);
    let mut upstream_req = format!(
        "GET {} HTTP/1.1\r\n\
Host: {}\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Key: {}\r\n\
Sec-WebSocket-Version: {}\r\n\
Origin: {}://{}\r\n",
        tpath, host_header, ws_key, ws_version, origin_val, host_header,
    );

    // 添加收集到的额外请求头（含 hometierproxy:// → http:// 重写）
    let upstream_base = format!("http://{}", host_header);
    for (key, value) in &extra_headers {
        let key_lower = key.to_lowercase();
        // 不重复添加已存在的头
        if key_lower == "sec-websocket-key"
            || key_lower == "sec-websocket-version" || key_lower == "host"
        {
            continue;
        }
        // 重写 Origin/Referer 中的 hometierproxy:// 为 http://
        let rewritten = if (key_lower == "referer" || key_lower == "origin") && value.starts_with("hometierproxy://") {
            if let Some(path_start) = value.find('/') {
                let after_scheme = &value[path_start + 1..];
                if let Some(slash_pos) = after_scheme.find('/') {
                    let path_and_qs = &after_scheme[slash_pos..];
                    format!("{}: {}", key, format!("{}{}", upstream_base, path_and_qs))
                } else {
                    format!("{}: {}", key, upstream_base)
                }
            } else {
                format!("{}: {}", key, value)
            }
        } else {
            format!("{}: {}", key, value)
        };
        let _ = writeln!(upstream_req, "{}", rewritten);
    }

    // 注入 Cookie（使用 cookie jar 中的持久化 cookie，其次使用 client 提供的）
    let host_key = format!("{}:{}", target_host, target_port);
    if let Some(jar_cookie) = hometier_protocol::cookie_jars().lock().unwrap()
        .entry(host_key.clone())
        .or_insert_with(hometier_protocol::PerHostCookieJar::new)
        .build_cookie_header()
    {
        let _ = writeln!(upstream_req, "Cookie: {}", jar_cookie);
    } else if let Some(ref cookie) = client_cookie {
        let _ = writeln!(upstream_req, "Cookie: {}", cookie);
    }

    upstream_req.push_str("\r\n");

    // 打印上游请求内容（仅前 800 字符避免日志膨胀）
    let req_preview = upstream_req.lines().take(20).collect::<Vec<_>>().join("\\n");
    crate::log_info!(format!("WS 代理: 上游请求内容:\n{}", req_preview));

    if let Err(e) = upstream.write_all(upstream_req.as_bytes()).await {
        crate::log_error!(format!("WS 代理: 发送上游请求失败 - {}", e));
        return;
    }
    crate::log_info!(format!("WS 代理: 上游请求已发送, 等待 101 响应"));

    // 7. 读取上游 101 响应
    let mut resp_buf = vec![0u8; 4096];
    let mut resp_total = 0;
    loop {
        let nr = match upstream.read(&mut resp_buf[resp_total..]).await {
            Ok(n) => n,
            Err(e) => {
                crate::log_error!(format!("WS 代理: 读取上游 101 响应失败 - {}", e));
                return;
            }
        };
        if nr == 0 {
            crate::log_warn!("WS 代理: 上游提前关闭连接");
            break;
        }
        resp_total += nr;
        if resp_buf[..resp_total].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if resp_total == resp_buf.len() {
            resp_buf.resize(resp_buf.len() + 4096, 0);
        }
    }

    let resp_data = &resp_buf[..resp_total];
    let resp_str = match std::str::from_utf8(resp_data) {
        Ok(s) => s,
        Err(_) => {
            crate::log_error!("WS 代理: 上游响应非 UTF-8");
            return;
        }
    };

    if !resp_str.contains("101") {
        let first_line = resp_str.lines().next().unwrap_or("(empty)");
        crate::log_error!(format!("WS 代理: 上游非 101 响应 - {}", first_line));
        return;
    }

    // 打印上游 101 响应的前 10 行
    let resp_preview: Vec<&str> = resp_str.lines().take(10).collect();
    crate::log_info!(format!("WS 代理: 上游 101 响应:\n{}", resp_preview.join("\n")));

    // 8. 提取 accept 值、leftover data，并捕获 Set-Cookie
    let resp_eoh = resp_data.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0);
    let resp_header_str = &resp_data[..resp_eoh];
    let accept_val = extract_header(
        std::str::from_utf8(resp_header_str).unwrap_or(""),
        "sec-websocket-accept",
    );
    let _up_leftover = resp_buf[resp_eoh + 4..resp_total].to_vec();

    // 9. 发送 101 响应到客户端（全量透传上游原始响应头和数据）
    crate::log_info!(format!("WS 代理: 发往客户端 101 (accept={})", accept_val));
    if let Err(e) = client.write_all(&resp_buf[..resp_total]).await {
        crate::log_error!(format!("WS 代理: 发送 101 响应到客户端失败 - {}", e));
        return;
    }
    crate::log_info!("WS 代理: 101 响应已发送, 开始双向数据转发");

    // 11. 双向数据转发
    bidirectional_copy(client, upstream).await;
}

/// 双向数据转发：client ↔ upstream
async fn bidirectional_copy(client: TcpStream, upstream: WsUpstream) {
    let (mut rc, mut wc) = tokio::io::split(client);
    let (mut ru, mut wu) = tokio::io::split(upstream);
    crate::log_info!("WS 代理: bidirectional_copy 开始执行");
    tokio::select! {
        result = tokio::io::copy(&mut rc, &mut wu) => {
            match result {
                Ok(n) => crate::log_info!(format!("WS 代理: client→upstream 转发完成, 共 {} 字节", n)),
                Err(e) => crate::log_error!(format!("WS 代理: client→upstream 转发失败 - {}", e)),
            }
        },
        result = tokio::io::copy(&mut ru, &mut wc) => {
            match result {
                Ok(n) => crate::log_info!(format!("WS 代理: upstream→client 转发完成, 共 {} 字节", n)),
                Err(e) => crate::log_error!(format!("WS 代理: upstream→client 转发失败 - {}", e)),
            }
        },
    }
    crate::log_info!("WS 代理: bidirectional_copy 结束");
}