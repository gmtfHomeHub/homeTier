use async_trait::async_trait;
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Bytes;

use crate::proxy::plugin::{ProxyHandler, ProxyResponse, RequestContext};

/// WebSocket proxy plugin using CONNECT-style tunnelling.
/// Handles Upgrade: websocket requests by establishing a TCP tunnel
/// to the target and upgrading the client connection.
pub struct WebSocketPlugin;

#[async_trait]
impl ProxyHandler for WebSocketPlugin {
    fn name(&self) -> &'static str {
        "websocket"
    }

    fn can_handle(&self, req: &Request<Incoming>) -> bool {
        // Check for WebSocket upgrade headers
        let is_upgrade = req
            .headers()
            .get("upgrade")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_lowercase() == "websocket")
            .unwrap_or(false);

        let is_connection_upgrade = req
            .headers()
            .get("connection")
            .and_then(|v| v.to_str().ok())
            .map(|v| {
                v.split(',')
                    .any(|part| part.trim().to_lowercase() == "upgrade")
            })
            .unwrap_or(false);

        is_upgrade && is_connection_upgrade
    }

    async fn handle(
        &self,
        _req: Request<Incoming>,
        _ctx: RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        // WebSocket upgrade is not yet fully implemented.
        // When the request enters via the /proxy?url= path, the original
        // Upgrade headers are forwarded to the upstream by the http_forward handler.
        //
        // TODO: Implement full WebSocket tunnel via tokio-tungstenite:
        //   1. Parse ?url= param for target
        //   2. Establish TCP/TLS connection to target
        //   3. Perform WebSocket handshake on upstream
        //   4. Upgrade client connection to WebSocket via hyper's upgrade API
        //   5. Bidirectional frame relay between client and upstream
        Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header("content-type", "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from(
                "WebSocket proxy not yet implemented",
            )))
            .unwrap())
    }
}
