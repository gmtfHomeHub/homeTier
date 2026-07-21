use async_trait::async_trait;
use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Bytes;

use crate::proxy::plugin::{ProxyHandler, ProxyResponse, RequestContext};

/// HTTPS CONNECT tunnel plugin.
/// Handles CONNECT requests by establishing a TCP tunnel
/// between the client and the target host:port.
pub struct HttpsTunnelPlugin;

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
        _req: Request<Incoming>,
        _ctx: RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        // CONNECT tunnel is not yet fully implemented as a plugin.
        // The HTTP forward handler can still proxy HTTPS URLs by forwarding
        // the request through reqwest (which handles TLS to upstream).
        //
        // TODO: Implement CONNECT tunnel:
        //   1. Parse host:port from URI (or Host header)
        //   2. Establish TCP connection to target
        //   3. Send "200 Connection Established" to client
        //   4. Bidirectional byte copy between client TCP and target TCP
        //   5. This requires access to the underlying TCP stream, which in hyper
        //      is done via http1::Builder's preserve_header_case() + on_incoming().
        //   6. Alternative: use hyper's low-level connection API.
        Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header("content-type", "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from(
                "CONNECT tunnel not yet implemented",
            )))
            .unwrap())
    }
}
