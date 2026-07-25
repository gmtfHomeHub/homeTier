use async_trait::async_trait;
use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::TokioIo;

use crate::proxy::plugin::{ProxyHandler, ProxyResponse, RequestContext};

/// HTTPS CONNECT tunnel plugin.
pub struct HttpsTunnelPlugin;

fn parse_host_port(uri: &hyper::Uri, host_header: Option<&str>) -> Option<(String, u16)> {
    let authority = uri
        .authority()
        .map(|a| a.to_string())
        .or_else(|| host_header.map(|h| h.to_string()));

    let authority = authority?;
    let mut parts = authority.rsplitn(2, ':');
    let port_str = parts.next()?;
    let host = parts.next().unwrap_or(&authority);

    let (host, port) = if port_str == host {
        (host, 443u16)
    } else {
        (host, port_str.parse::<u16>().unwrap_or(443))
    };

    let host = host.trim_matches(|c| c == '[' || c == ']');
    if host.is_empty() {
        return None;
    }

    Some((host.to_string(), port))
}

fn build_response(status: StatusCode) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Full::new(Bytes::new()))
        .unwrap()
}

#[async_trait]
impl ProxyHandler for HttpsTunnelPlugin {
    fn name(&self) -> &'static str {
        "https_tunnel"
    }

    fn can_handle(&self, req: &Request<Incoming>) -> bool {
        req.method() == Method::CONNECT
    }

    async fn handle(
        &self,
        req: Request<Incoming>,
        _ctx: crate::proxy::plugin::RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        let uri = req.uri().clone();
        let host_header = req
            .headers()
            .get("host")
            .and_then(|v| v.to_str().ok().map(|s| s.to_string()));

        let (host, port) = match parse_host_port(
            &uri,
            host_header.as_deref(),
        ) {
            Some(hp) => hp,
            None => {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header("content-type", "text/plain; charset=utf-8")
                    .body(Full::new(Bytes::from("Invalid CONNECT target")))
                    .unwrap());
            }
        };

        match tokio::net::TcpStream::connect(format!("{}:{}", host, port)).await {
            Ok(upstream) => {
                let on_upgrade: OnUpgrade = hyper::upgrade::on(req);

tokio::spawn(async move {
                    if let Ok(upgraded) = on_upgrade.await {
                        let mut client_io = TokioIo::new(upgraded);
                        let mut upstream_io = TokioIo::new(upstream);
                        let _ = tokio::io::copy_bidirectional(
                            &mut client_io,
                            &mut upstream_io,
                        ).await;
                    }
                });

                Ok(build_response(StatusCode::OK))
            }
            Err(e) => {
                crate::log_warn!(format!("CONNECT tunnel failed: {}:{} -> {}", host, port, e));
                Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .header("content-type", "text/plain; charset=utf-8")
                    .body(Full::new(Bytes::from(format!("Failed to connect to {}:{}", host, port))))
                    .unwrap())
            }
        }
    }
}