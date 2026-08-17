use std::fmt::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::proxy::hometier_protocol;  // 用于共享 CookieJar
use crate::proxy::ProxyKeyMap;

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
        let mut roots = rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
        );
        for cert in crate::proxy::proxy_ca_der() {
            let _ = roots.add(cert);
        }
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
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
/// `key_map`: __proxy__{key} → 源 URL 映射（用于站点 JS 用 location 拼出的“自身”WS 地址解析真实上游）
/// `front_port`: 本代理前置端口（禁止作为上游连接，防止自环递归）
pub async fn handle_raw_upgrade(
    mut client: TcpStream,
    initial_data: Vec<u8>,
    key_map: ProxyKeyMap,
    front_port: u16,
) {
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

    // 解析 __proxy__{key} 前缀：站点 JS 基于 location 拼接的自引用地址，
    // 目标可能指向前置端口自身，需用 key 还原真实上游（见下方步骤 5）。
    let key_origin: Option<String> = {
        let path_noq = path.split('?').next().unwrap_or(path);
        if let Some(rest) = path_noq.strip_prefix("/__proxy__") {
            let key = rest.split('/').next().unwrap_or("");
            if !key.is_empty() {
                key_map.read().await.get(key).cloned()
            } else {
                None
            }
        } else {
            None
        }
    };

    // 3. 从查询参数解析 scheme 和目标地址 (格式: ?ws=encoded_target 或 ?wss=encoded_target)
    let (mut scheme, encoded_target) = if let Some(qs) = path.split('?').nth(1) {
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
                // 以下 WebSocket 无关头透传到上游可能导致服务端(如 Getv/String)
                // 误用 origin-ish header 构建路径/namespace——在共享页返回错误的
                // 具体示例：CasaOS 的 socket.io 会根据 Referer 生成 40//host... 损坏路径
                // 纯转发 WS 要求：这些头完全不应上传
                // accept-encoding: 剥离后上游(CasaOS Go gzip 中间件等)不会对 101 升级
                // 响应错误启用 gzip——WS 帧传输与 HTTP 压缩无关(permessage-deflate 已剥离)
                "referer" | "pragma" | "cache-control"
                | "sec-fetch-site" | "sec-fetch-mode" | "sec-fetch-dest"
                | "accept-encoding" => {}
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

    // 5. 自引用目标还原：若目标解析为代理前置自身（站点 JS 用 location 拼接），
    // 通过 __proxy__{key} 映射的源 URL 还原真实上游 host（path 保持不变）。
    if let Some(origin) = key_origin {
        let is_self = matches!(target_host.as_str(), "127.0.0.1" | "localhost" | "::1" | "[::1]")
            && target_port == front_port;
        if is_self || !target_host_port_valid {
            let origin_rest = origin
                .split_once("://")
                .map(|(_, rest)| rest)
                .unwrap_or(origin.as_str());
            let authority = origin_rest.split('/').next().unwrap_or(origin_rest);
            match authority.rfind(':') {
                Some(colon) => {
                    if let Ok(port) = authority[colon + 1..].parse::<u16>() {
                        crate::log_info!(format!(
                            "WS 代理: 自引用目标由 key 还原 → {}:{} (path 保持 {})",
                            &authority[..colon], port, tpath
                        ));
                        target_host = authority[..colon].to_string();
                        target_port = port;
                        if origin.starts_with("https://") {
                            scheme = "wss";
                        }
                    }
                }
                None => {
                    target_host = authority.to_string();
                    if origin.starts_with("https://") {
                        target_port = 443;
                        scheme = "wss";
                    } else {
                        target_port = 80;
                    }
                }
            }
        }
    }

    // 硬性防自环：任何情况下都禁止把前置端口自身作为上游连接。
    if matches!(target_host.as_str(), "127.0.0.1" | "localhost" | "::1" | "[::1]")
        && target_port == front_port
    {
        crate::log_error!(format!("WS 代理: 阻止自环连接 {}:{}", target_host, target_port));
        send_error(&mut client, "502 Bad Gateway", "self-connect blocked").await;
        return;
    }

    // 6. 连接上游
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
    // Origin 只发一次：优先使用客户端 Origin，缺失时使用默认值。
    // 避免上游收到重复 Origin 头被拒绝（如 nginx 400）。
    let upstream_origin = match client_origin.as_deref() {
        Some(o) => o.to_string(),
        None => format!("{}://{}", if scheme == "wss" { "https" } else { "http" }, host_header),
    };
    crate::log_info!(format!(
        "WS 代理: Origin 重写 → 客户端 client_origin={:?} → upstream_origin={}",
        client_origin, upstream_origin
    ));
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

    // 添加收集到的额外请求头
    for (key, value) in &extra_headers {
        let key_lower = key.to_lowercase();
        // 不重复添加已存在的头
        if key_lower == "sec-websocket-key"
            || key_lower == "sec-websocket-version" || key_lower == "host"
        {
            continue;
        }
        let _ = write!(upstream_req, "{}: {}\r\n", key, value);
    }

    // 注入 Cookie（使用 cookie jar 中的持久化 cookie，其次使用 client 提供的）
    let host_key = format!("{}:{}", target_host, target_port);
    let jar_cookie = hometier_protocol::cookie_jars()
        .lock()
        .unwrap()
        .entry(host_key.clone())
        .or_insert_with(hometier_protocol::PerHostCookieJar::new)
        .build_cookie_header(&target_host, "/");
    if let Some(ref jar_cookie) = jar_cookie {
        crate::log_info!("WS 代理: Cookie 注入来源=jar(持久化), 优先于客户端请求头");
        let _ = write!(upstream_req, "Cookie: {}\r\n", jar_cookie);
    } else if let Some(ref cookie) = client_cookie {
        crate::log_info!("WS 代理: Cookie 注入来源=客户端请求头");
        let _ = write!(upstream_req, "Cookie: {}\r\n", cookie);
    } else {
        crate::log_info!("WS 代理: Cookie 注入来源=无");
    }

    upstream_req.push_str("\r\n");

    // 打印上游请求内容（仅前 800 字符避免日志膨胀）
    let req_preview = upstream_req.lines().take(20).collect::<Vec<_>>().join("\\n");
    crate::log_info!(format!("WS 代理: 上游请求内容:\n{}", req_preview));
    let origin_line = upstream_req.lines().find(|l| l.to_lowercase().starts_with("origin")).unwrap_or("(none)");
    crate::log_info!(format!("WS 代理: 诊断 Origin={} client_origin_raw={:?}", origin_line, client_origin));

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
    crate::log_info!(format!("WS 代理: 上游响应总字节={}", resp_total));

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

    // 诊断: 101 响应是否携带 content-encoding(异常——升级响应不应有表示元数据头,
    // 如 CasaOS Go gzip 中间件错误附加的 content-encoding: gzip)
    if resp_str.lines().any(|l| l.to_lowercase().starts_with("content-encoding:")) {
        crate::log_warn!("WS 代理: 上游 101 响应含 content-encoding 头(异常)——服务器可能对 WS 连接启用了 HTTP gzip, 该头将在透传时清洗");
    }

    // 诊断: leftover 帧数据 hex dump + gzip 魔数检测(1f 8b)——判断 101 后紧随的数据
    // 是否被 gzip 污染(若为 gzip 流则客户端按 WS 帧解析会协议错误并关闭连接)
    let leftover_start = resp_eoh + 4;
    let leftover = &resp_data[leftover_start..];
    if !leftover.is_empty() {
        let hex_head: String = leftover
            .iter()
            .take(16)
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let gzip_magic = leftover.len() >= 2 && leftover[0] == 0x1f && leftover[1] == 0x8b;
        crate::log_info!(format!(
            "WS 代理: leftover 帧数据={} 字节 hex(前16)=[{}] gzip魔数={}",
            leftover.len(),
            hex_head,
            gzip_magic
        ));
    }

    // 8. 提取 accept 值
    let accept_val = extract_header(resp_str, "sec-websocket-accept");

    // 9. 清洗 101 响应头后发送到客户端：剔除 HTTP 表示/传输元数据头
    //    (content-encoding/content-length/transfer-encoding/vary 等——101 升级响应
    //     不应携带, 如 CasaOS 的 gzip 中间件错误附加的 content-encoding: gzip),
    //    保留 WS 必需头(upgrade/connection/sec-websocket-accept)与会话头(set-cookie)。
    //    Sec-WebSocket-Accept 由客户端直接校验, 代理不重算, 原值透传即通过。
    let strip_resp_headers = [
        "content-encoding",
        "content-length",
        "transfer-encoding",
        "vary",
        "trailer",
        "trailers",
    ];
    let mut filtered_resp: Vec<&str> = Vec::new();
    for line in resp_str.lines() {
        let lower = line.to_lowercase();
        let skip = strip_resp_headers
            .iter()
            .any(|h| lower.starts_with(&format!("{}:", h)));
        if skip {
            continue;
        }
        filtered_resp.push(line);
    }
    let filtered_head = filtered_resp.join("\r\n");

    crate::log_info!(format!(
        "WS 代理: 发往客户端 101 (accept={}) 清洗后头={}行/{}字节",
        accept_val,
        filtered_resp.len(),
        filtered_head.len()
    ));
    let mut out = Vec::with_capacity(filtered_head.len() + 4 + leftover.len());
    out.extend_from_slice(filtered_head.as_bytes());
    out.extend_from_slice(b"\r\n\r\n");
    out.extend_from_slice(leftover);
    if let Err(e) = client.write_all(&out).await {
        crate::log_error!(format!("WS 代理: 发送 101 响应到客户端失败 - {}", e));
        return;
    }
    crate::log_info!(format!(
        "WS 代理: 发往客户端响应: 头部={} 字节, leftover帧数据={} 字节",
        filtered_head.len() + 4,
        leftover.len()
    ));
    crate::log_info!("WS 代理: 开始双向数据转发");

    // 11. 双向数据转发
    bidirectional_copy(client, upstream).await;
}

/// 双向数据转发：client ↔ upstream
/// 两个方向各自独立运行：任一侧 EOF 只对其对端执行 half-close（TCP 半关闭）。
/// 任一侧先结束(EOF/错误)后, 立即半关对端写端并终止另一侧任务, 避免连接挂起泄漏
/// (原 join! 在客户端不关闭时会永久等待上游方向)。
async fn bidirectional_copy(client: TcpStream, upstream: WsUpstream) {
    let (mut rc, mut wc) = tokio::io::split(client);
    let (mut ru, mut wu) = tokio::io::split(upstream);
    crate::log_info!("WS 代理: bidirectional_copy 开始执行");

    // 方向 1: client → upstream
    let mut c2u_h = tokio::spawn(async move {
        match tokio::io::copy(&mut rc, &mut wu).await {
            Ok(0) => crate::log_info!("WS 代理: client→upstream EOF (客户端关闭, 未转发数据)"),
            Ok(n) => crate::log_info!(format!(
                "WS 代理: client→upstream 转发完成, 共 {} 字节 (客户端关闭)",
                n
            )),
            Err(e) => crate::log_error!(format!("WS 代理: client→upstream 转发失败 - {}", e)),
        }
        // client 侧 EOF：半关上游写端，通知上游数据结束
        let _ = wu.shutdown().await;
    });

    // 方向 2: upstream → client
    let mut u2c_h = tokio::spawn(async move {
        match tokio::io::copy(&mut ru, &mut wc).await {
            Ok(0) => crate::log_info!("WS 代理: upstream→client EOF (上游关闭, 未转发数据)"),
            Ok(n) => crate::log_info!(format!(
                "WS 代理: upstream→client 转发完成, 共 {} 字节 (上游关闭)",
                n
            )),
            Err(e) => crate::log_error!(format!("WS 代理: upstream→client 转发失败 - {}", e)),
        }
        // 上游侧已关闭，半关客户端写端，客户端读到 FIN 自行结束
        let _ = wc.shutdown().await;
    });

    // 等待先结束的方向：终止另一侧任务, 避免连接挂起泄漏
    tokio::select! {
        _ = &mut c2u_h => {
            crate::log_info!("WS 代理: client→upstream 方向先完成, 终止 upstream→client");
        }
        _ = &mut u2c_h => {
            crate::log_info!("WS 代理: upstream→client 方向先完成, 终止 client→upstream");
        }
    }
    c2u_h.abort();
    u2c_h.abort();
    crate::log_info!("WS 代理: bidirectional_copy 结束");
}