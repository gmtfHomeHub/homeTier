use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

enum WsUpstream {
    Plain(TcpStream),
    Tls(tokio_rustls::TlsStream<TcpStream>),
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

pub fn is_ws_upgrade(buf: &[u8]) -> bool {
    let s = match std::str::from_utf8(buf) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if !s.starts_with("GET") && !s.starts_with("get") {
        return false;
    }
    let lower = s.to_lowercase();
    lower.contains("upgrade:") && lower.contains("websocket")
}

pub async fn handle_stream(
    stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut reader, mut writer) = tokio::io::split(stream);

    let mut buf = vec![0u8; 8192];
    let n = reader
        .read(&mut buf)
        .await
        .map_err(|e| format!("read ws request: {}", e))?;
    if n == 0 {
        return Err("connection closed before ws request".into());
    }
    let data = &buf[..n];

    let eoh = data
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "incomplete ws headers".to_string())?;
    let header_str = std::str::from_utf8(&data[..eoh])
        .map_err(|_| "invalid utf-8 in ws request".to_string())?;

    let mut lines = header_str.lines();
    let request_line = lines.next().ok_or("missing request line")?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err("invalid request line".into());
    }
    let path = parts[1];

    let (scheme, rest) = if let Some(r) = path.strip_prefix("/ws/") {
        ("ws", r)
    } else if let Some(r) = path.strip_prefix("/wss/") {
        ("wss", r)
    } else {
        return Err(format!("invalid ws proxy path: {}", path).into());
    };

    let (authority, tpath) = match rest.find('/') {
        Some(pos) => (&rest[..pos], &rest[pos..]),
        None => (rest, "/"),
    };
    let default_port: u16 = if scheme == "wss" { 443 } else { 80 };
    let (target_host, target_port) = match authority.rfind(':') {
        Some(pos) => (
            authority[..pos].to_string(),
            authority[pos + 1..].parse().unwrap_or(default_port),
        ),
        None => (authority.to_string(), default_port),
    };

    let mut ws_key = String::new();
    let mut ws_version = String::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            match k.trim().to_lowercase().as_str() {
                "sec-websocket-key" => ws_key = v.trim().to_string(),
                "sec-websocket-version" => ws_version = v.trim().to_string(),
                _ => {}
            }
        }
    }

    let upstream_req = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}:{}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: {}\r\n\
         Origin: {}://{}:{}\r\n\
         \r\n",
        tpath,
        target_host,
        target_port,
        ws_key,
        ws_version,
        if scheme == "wss" { "https" } else { "http" },
        target_host,
        target_port,
    );

    let bare = timeout(
        Duration::from_secs(15),
        TcpStream::connect(format!("{}:{}", target_host, target_port)),
    )
    .await
    .map_err(|_| format!("connect upstream {}:{}: timeout", target_host, target_port))?
    .map_err(|e| format!("connect upstream {}:{}: {}", target_host, target_port, e))?;

    let mut upstream: WsUpstream = if scheme == "wss" {
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore::from_iter(
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
            ))
            .with_no_client_auth();
        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
        let domain = rustls::pki_types::ServerName::try_from(target_host.clone())
            .map_err(|_| format!("invalid hostname: {}", target_host))?;
        let tls_stream = connector
            .connect(domain, bare)
            .await
            .map_err(|e| format!("tls connect: {}", e))?;
        WsUpstream::Tls(tls_stream)
    } else {
        WsUpstream::Plain(bare)
    };

    upstream
        .write_all(upstream_req.as_bytes())
        .await
        .map_err(|e| format!("send upstream ws upgrade: {}", e))?;

    let mut resp_buf = vec![0u8; 4096];
    let mut resp_total = 0;
    loop {
        let nr = upstream
            .read(&mut resp_buf[resp_total..])
            .await
            .map_err(|e| format!("read upstream response: {}", e))?;
        if nr == 0 {
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
    let resp_str =
        std::str::from_utf8(resp_data).map_err(|_| "invalid utf-8 upstream response")?;

    if !resp_str.contains("101") {
        let _ = writer.write_all(resp_data).await;
        return Ok(());
    }

    let resp_eoh = resp_data
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap();
    let resp_header_str =
        std::str::from_utf8(&resp_data[..resp_eoh]).map_err(|_| "invalid utf-8 headers")?;

    let mut sec_ws_accept = String::new();
    for line in resp_header_str.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().to_lowercase() == "sec-websocket-accept" {
                sec_ws_accept = v.trim().to_string();
                break;
            }
        }
    }

    let client_resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\
         \r\n",
        sec_ws_accept
    );
    writer
        .write_all(client_resp.as_bytes())
        .await
        .map_err(|e| format!("send 101 to client: {}", e))?;

    let client_leftover = &data[eoh + 4..n];
    if !client_leftover.is_empty() {
        let _ = upstream.write_all(client_leftover).await;
    }

    let up_leftover = &resp_buf[resp_eoh + 4..resp_total];
    if !up_leftover.is_empty() {
        let _ = writer.write_all(up_leftover).await;
    }

    let (mut up_reader, mut up_writer) = tokio::io::split(upstream);

    tokio::select! {
        _ = tokio::io::copy(&mut reader, &mut up_writer) => {},
        _ = tokio::io::copy(&mut up_reader, &mut writer) => {},
    };

    Ok(())
}
