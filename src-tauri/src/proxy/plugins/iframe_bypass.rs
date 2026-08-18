use async_trait::async_trait;

use crate::proxy::plugin::{ProxyPlugin, ProxyResponse, RequestContext};

pub struct IframeBypassPlugin;

#[async_trait]
impl ProxyPlugin for IframeBypassPlugin {
    fn name(&self) -> &'static str {
        "iframe_bypass"
    }

    fn priority(&self) -> i32 {
        20
    }

    async fn post_process(
        &self,
        mut resp: ProxyResponse,
        _ctx: &RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        let headers = resp.headers_mut();

        // Strip X-Frame-Options (all variants)
        headers.remove("x-frame-options");

        // Filter frame-ancestors from Content-Security-Policy
        if let Some(csp) = headers.get("content-security-policy") {
            if let Ok(val) = csp.to_str() {
                let filtered: Vec<&str> = val
                    .split(';')
                    .map(|s| s.trim())
                    .filter(|s| !s.starts_with("frame-ancestors"))
                    .collect();
                let joined = filtered.join("; ");
                if joined.is_empty() {
                    headers.remove("content-security-policy");
                } else {
                    headers.insert(
                        "content-security-policy",
                        joined.parse().unwrap(),
                    );
                }
            }
        }

        Ok(resp)
    }
}
