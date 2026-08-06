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
    let bare = timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("connect timeout: {}", addr))?
        .map_err(|e| format!("connect failed: {}: {}", addr, e))?;

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

/// 上游连接/握手失败时，向客户端发送明确的 HTTP 错误响应后关闭连接，
/// 避免 WebView 侧只看到 "Socket is not connected"
async fn send_error(client: &mut TcpStream, status_line: &str, message: &str) {
    let body = format!("WebSocket proxy error: {}\n", message);
    let resp = format!(
        "HTTP/1.1 {}\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\
        \r\n\
        {}",
        status_line,
        body.as_bytes().len(),
        body
    );
    let _ = client.write_all(resp.as_bytes()).await;
}

/// 直接从 TcpStream 处理 WebSocket 升级请求（无需 hyper upgrade 机制）。
/// 接收原始 TCP 流和已预读的请求数据，解析 HTTP 请求，连接上游，发送 101 响应，然后双向转发数据。
pub async fn handle_raw_upgrade(mut client: TcpStream, initial_data: Vec<u8>) {
    // 1. 使用外部传入的初始数据解析 HTTP 请求（数据由 server.rs 预读传入）
    let data = &initial_data[..];

    // 定位 \r\n\r\n（请求头结束）
    let eoh = match data.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(p) => p,
        None => {
            send_error(&mut client, "400 Bad Request", "incomplete request headers").await;
            return;
        }
    };
    let header_str = match std::str::from_utf8(&data[..eoh]) {
        Ok(s) => s,
        Err(_) => {
            send_error(&mut client, "400 Bad Request", "request headers not utf-8").await;
            return;
        }
    };

    // 2. 解析请求行
    let request_line = match header_str.lines().next() {
        Some(l) => l,
        None => {
            send_error(&mut client, "400 Bad Request", "empty request line").await;
            return;
        }
    };
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        send_error(&mut client, "400 Bad Request", "malformed request line").await;
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
            send_error(&mut client, "400 Bad Request", &format!("unsupported query: {}", path)).await;
            return;
        }
    } else {
        crate::log_error!(format!("WS 代理: 缺少查询参数: {}", path));
        send_error(&mut client, "400 Bad Request", "missing ?ws= query").await;
        return;
    };

    // URL 解码目标地址
    let target_str = match urlencoding::decode(encoded_target) {
        Ok(s) => s.into_owned(),
        Err(_) => {
            crate::log_error!(format!("WS 代理: URL 解码失败: {}", encoded_target));
            send_error(&mut client, "400 Bad Request", "url decode failed").await;
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
    let mut client_origin: Option<String> = None;

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
                "origin" => client_origin = Some(value),
                // 剥离 permessage-deflate 等扩展协商：上游(如 DSM)不支持时会在握手后立即关闭。
                // WebSocket 帧透传代理无法参与扩展协商，主动丢弃最稳妥。
                "sec-websocket-extensions" => {}
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
        if let Some(ref origin) = client_origin {
            let origin_str = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")).unwrap_or("");
            if let Some(colon) = origin_str.rfind(':') {
                let host = &origin_str[..colon];
                if let Ok(port) = origin_str[colon + 1..].parse::<u16>() {
                    crate::log_info!(format!("WS 代理: 从 Origin 头提取 target={}:{}", host, port));
                    target_host = host.to_string();
                    target_port = port;
                }
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
            send_error(&mut client, "502 Bad Gateway", &e).await;
            return;
        }
    };

    // 6. 发送 WS upgrade 请求到上游（包含所有转发头 + cookie）
    let host_header = format!("{}:{}", target_host, target_port);
    let upstream_base = format!("http://{}", host_header);
    // Origin 只发一次：优先使用客户端 Origin（hometierproxy:// 重写为 http://），
    // 缺失时使用默认值。避免上游收到重复 Origin 头被拒绝（如 nginx 400）。
    let upstream_origin = match client_origin.as_deref() {
        Some(o) if o.starts_with("hometierproxy://") => {
            // 精确去掉协议前缀，剩余 host[:port][/path]；find('/') 直接定位路径起始，
            // 避免从第二个斜杠开始切导致 "http://host:port//host:port" 的损坏 Origin
            let rest = &o["hometierproxy://".len()..];
            match rest.find('/') {
                Some(slash_pos) => format!("{}{}", upstream_base, &rest[slash_pos..]),
                None => upstream_base.clone(),
            }
        }
        Some(o) => o.to_string(),
        None => format!("{}://{}", if scheme == "wss" { "https" } else { "http" }, host_header),
    };
    let mut upstream_req = format!(
        "GET {} HTTP/1.1\r\n\
        Host: {}\r\n\
        Upgrade: websocket\r\n\
        Connection: Upgrade\r\n\
        Sec-WebSocket-Key: {}\r\n\
        Sec-WebSocket-Version: {}\r\n\
        Origin: {}\r\n",
        tpath, host_header, ws_key, ws_version, upstream_origin,
    );

    // 添加收集到的额外请求头（含 Referer 的 hometierproxy:// → http:// 重写）
    for (key, value) in &extra_headers {
        let key_lower = key.to_lowercase();
        // 不重复添加已存在的头
        if key_lower == "sec-websocket-key"
            || key_lower == "sec-websocket-version" || key_lower == "host"
        {
            continue;
        }
        // 重写 Referer 中的 hometierproxy:// 为 http://
        let rewritten = if key_lower == "referer" && value.starts_with("hometierproxy://") {
            let rest = &value["hometierproxy://".len()..];
            match rest.find('/') {
                Some(slash_pos) => format!("{}: {}", key, format!("{}{}", upstream_base, &rest[slash_pos..])),
                None => format!("{}: {}", key, upstream_base),
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
        send_error(&mut client, "502 Bad Gateway", &format!("send upstream request failed: {}", e)).await;
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
                send_error(&mut client, "502 Bad Gateway", &format!("read upstream response failed: {}", e)).await;
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

    // 定位响应头结束位置（\r\n\r\n）。上游可能在 101 后立即推送 WebSocket 帧，
    // 帧为二进制数据，必须与响应头分开处理，不能对整段缓冲做 UTF-8 校验。
    let resp_eoh = match resp_data.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(p) => p,
        None => {
            let first = String::from_utf8_lossy(resp_data)
                .lines()
                .next()
                .unwrap_or("(empty)")
                .to_string();
            crate::log_error!(format!("WS 代理: 上游响应缺少头部结束符 - {}", first));
            send_error(&mut client, "502 Bad Gateway", "upstream response missing header terminator").await;
            return;
        }
    };
    let resp_header_str = &resp_data[..resp_eoh];

    // 只对响应头（纯 ASCII/UTF-8）做 UTF-8 校验与 101 检查
    let resp_str = match std::str::from_utf8(resp_header_str) {
        Ok(s) => s,
        Err(_) => {
            crate::log_error!("WS 代理: 上游响应头非 UTF-8");
            send_error(&mut client, "502 Bad Gateway", "upstream response header not utf-8").await;
            return;
        }
    };

    if !resp_str.contains("101") {
        let first_line = resp_str.lines().next().unwrap_or("(empty)");
        crate::log_error!(format!("WS 代理: 上游非 101 响应 - {}", first_line));
        send_error(&mut client, "502 Bad Gateway", &format!("upstream rejected upgrade: {}", first_line)).await;
        return;
    }

    // 打印上游 101 响应的前 10 行
    let resp_preview: Vec<&str> = resp_str.lines().take(10).collect();
    crate::log_info!(format!("WS 代理: 上游 101 响应:\n{}", resp_preview.join("\n")));

    // 8. 提取 accept 值
    let accept_val = extract_header(resp_str, "sec-websocket-accept");

    // 9. 发送 101 响应到客户端（全量透传上游原始响应头；残留的 WebSocket 帧数据一并下发）
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
/// 两个方向各自独立运行：任一侧 EOF 只对其对端执行 half-close（TCP 半关闭），
/// 不中断另一侧，避免 select 竞态导致客户端被 RST。
async fn bidirectional_copy(client: TcpStream, upstream: WsUpstream) {
    let (mut rc, mut wc) = tokio::io::split(client);
    let (mut ru, mut wu) = tokio::io::split(upstream);
    crate::log_info!("WS 代理: bidirectional_copy 开始执行");

    // 方向 1: client → upstream
    let c2u = tokio::spawn(async move {
        match tokio::io::copy(&mut rc, &mut wu).await {
            Ok(n) => crate::log_info!(format!("WS 代理: client→upstream 转发完成, 共 {} 字节", n)),
            Err(e) => crate::log_error!(format!("WS 代理: client→upstream 转发失败 - {}", e)),
        }
        // client 侧 EOF：半关上游写端，通知上游数据结束
        let _ = wu.shutdown().await;
    });

    // 方向 2: upstream → client
    let u2c = tokio::spawn(async move {
        match tokio::io::copy(&mut ru, &mut wc).await {
            Ok(n) => crate::log_info!(format!("WS 代理: upstream→client 转发完成, 共 {} 字节", n)),
            Err(e) => crate::log_error!(format!("WS 代理: upstream→client 转发失败 - {}", e)),
        }
        // 上游侧已关闭，半关客户端写端，客户端读到 FIN 自行结束
        let _ = wc.shutdown().await;
    });

    let _ = tokio::join!(c2u, u2c);
    crate::log_info!("WS 代理: bidirectional_copy 结束");
}