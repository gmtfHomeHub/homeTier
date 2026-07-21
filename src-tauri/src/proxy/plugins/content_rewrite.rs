use async_trait::async_trait;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

use crate::proxy::plugin::{ProxyPlugin, ProxyResponse, RequestContext, ResponseBody};
use crate::proxy::rewriter::{classify_content, detect_charset, rewrite_urls};

pub struct ContentRewriterPlugin {
    proxy_prefix: Option<String>,
}

impl ContentRewriterPlugin {
    pub fn new(proxy_prefix: Option<String>) -> Self {
        Self { proxy_prefix }
    }
}

#[async_trait]
impl ProxyPlugin for ContentRewriterPlugin {
    fn name(&self) -> &'static str {
        "content_rewrite"
    }

    fn priority(&self) -> i32 {
        30
    }

    async fn post_process(
        &self,
        resp: ProxyResponse,
        ctx: &RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        if !ctx.should_rewrite {
            return Ok(resp);
        }

        let proxy_prefix = match &self.proxy_prefix {
            Some(p) => p.clone(),
            None => return Ok(resp),
        };

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let target_url = match &ctx.target_url {
            Some(u) => u.clone(),
            None => return Ok(resp),
        };

        match classify_content(&content_type) {
            crate::proxy::rewriter::RewriteTarget::Html
            | crate::proxy::rewriter::RewriteTarget::Css => {}
            _ => return Ok(resp),
        }

        let (mut parts, body) = resp.into_parts();
        let body_bytes = body.into_inner();

        let encoding = detect_charset(&content_type);
        let (body_str, _, _) = encoding.decode(&body_bytes);

        let rewritten =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rewrite_urls(&body_str, &target_url, &proxy_prefix)
            }));

        match rewritten {
            Ok(cow) => {
                let new_bytes = cow.as_bytes().to_vec();
                parts
                    .headers
                    .insert("content-length", new_bytes.len().into());
                Ok(Response::from_parts(parts, Full::new(Bytes::from(new_bytes))))
            }
            Err(_) => Ok(Response::from_parts(parts, Full::new(body_bytes))),
        }
    }
}
