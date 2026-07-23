use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};
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

fn error_response(status: u16, msg: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .header("access-control-allow-origin", "*")
        .body(Full::new(Bytes::from(msg.to_string())))
        .unwrap()
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

pub fn is_ws_upgrade(req: &Request<Incoming>) -> bool {
    req.method() == hyper::Method::GET
        && req
            .headers()
            .get("upgrade")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_lowercase().contains("websocket"))
            .unwrap_or(false)
        && req
            .headers()
            .get("connection")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_lowercase().contains("upgrade"))
            .unwrap_or(false)
}

pub async fn handle_upgrade(req: Request<Incoming>) -> Response<Full<Bytes>> {
    let (parts, body) = req.into_parts();

    let path = parts.uri.path();
    let (scheme, rest) = if let Some(r) = path.strip_prefix("/ws/") {
        ("ws", r)
    } else if let Some(r) = path.strip_prefix("/wss/") {
        ("wss", r)
    } else {
        return error_response(400, &format!("invalid ws proxy path: {}", path));
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

    let ws_key = parts
        .headers
        .get("sec-websocket-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let ws_version = parts
        .headers
        .get("sec-websocket-version")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("13")
        .to_string();

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

    let mut upstream = match connect_upstream(&target_host, target_port, scheme).await {
        Ok(u) => u,
        Err(e) => return error_response(502, &e),
    };

    if let Err(e) = upstream.write_all(upstream_req.as_bytes()).await {
        return error_response(502, &format!("send upstream: {}", e));
    }

    // Read upstream response (HTTP headers only, rest is WS frames)
    let mut resp_buf = vec![0u8; 4096];
    let mut resp_total = 0;
    loop {
        let nr = match upstream.read(&mut resp_buf[resp_total..]).await {
            Ok(n) => n,
            Err(e) => return error_response(502, &format!("read upstream response: {}", e)),
        };
        if nr == 0 {
            break;
        }
        resp_total += nr;
        if resp_buf[..resp_total]
            .windows(4)
            .any(|w| w == b"\r\n\r\n")
        {
            break;
        }
        if resp_total == resp_buf.len() {
            resp_buf.resize(resp_buf.len() + 4096, 0);
        }
    }

    let resp_data = &resp_buf[..resp_total];
    let resp_str = match std::str::from_utf8(resp_data) {
        Ok(s) => s,
        Err(_) => return error_response(502, "invalid utf-8 upstream response"),
    };

    if !resp_str.contains("101") {
        return Response::builder()
            .status(200)
            .body(Full::new(Bytes::from(resp_data.to_vec())))
            .unwrap();
    }

    let resp_eoh = resp_data
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap();
    let resp_header_str = match std::str::from_utf8(&resp_data[..resp_eoh]) {
        Ok(s) => s,
        Err(_) => return error_response(502, "invalid utf-8 upstream headers"),
    };

    let accept_val = extract_header(resp_header_str, "sec-websocket-accept");
    let up_leftover = resp_buf[resp_eoh + 4..resp_total].to_vec();

    // Build the 101 response and return it.
    // Hyper will handle the connection upgrade automatically.
    Response::builder()
        .status(101)
        .header("upgrade", "websocket")
        .header("connection", "upgrade")
        .header("sec-websocket-accept", accept_val)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

async fn connect_upstream(
    target_host: &str,
    target_port: u16,
    scheme: &str,
) -> Result<WsUpstream, String> {
    // Primary attempt: connect to target_host directly
    let addr = format!("{}:{}", target_host, target_port);
    let bare = match timeout(Duration::from_secs(10), TcpStream::connect(&addr)).await {
        Ok(Ok(s)) => s,
        _ => {
            // Fallback: try 127.0.0.1 with the same port (for .sock /
            // internal hostnames that aren't TCP-resolvable)
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
        Ok(WsUpstream::Tls(tokio_rustls::TlsStream::Client(tls_stream)))
    } else {
        Ok(WsUpstream::Plain(bare))
    }
}
