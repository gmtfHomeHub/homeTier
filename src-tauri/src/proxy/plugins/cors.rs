use async_trait::async_trait;
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::plugin::{ProxyPlugin, ProxyResponse, RequestContext};

pub struct CorsPlugin {
    allowed_origins: String,
    allowed_methods: String,
    allowed_headers: String,
}

impl Default for CorsPlugin {
    fn default() -> Self {
        Self {
            allowed_origins: "*".to_string(),
            allowed_methods: "GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS".to_string(),
            allowed_headers: "*".to_string(),
        }
    }
}

impl CorsPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_origins(mut self, origins: &str) -> Self {
        self.allowed_origins = origins.to_string();
        self
    }

    pub fn with_methods(mut self, methods: &str) -> Self {
        self.allowed_methods = methods.to_string();
        self
    }

    pub fn with_headers(mut self, headers: &str) -> Self {
        self.allowed_headers = headers.to_string();
        self
    }
}

#[async_trait]
impl ProxyPlugin for CorsPlugin {
    fn name(&self) -> &'static str {
        "cors"
    }

    fn priority(&self) -> i32 {
        10
    }

    async fn pre_process(
        &self,
        req: &mut Request<Incoming>,
        _ctx: &mut RequestContext,
    ) -> Result<Option<ProxyResponse>, Box<dyn std::error::Error + Send + Sync>> {
        if req.method() == Method::OPTIONS {
            let resp = Response::builder()
                .status(StatusCode::NO_CONTENT)
                .header("access-control-allow-origin", &self.allowed_origins)
                .header("access-control-allow-methods", &self.allowed_methods)
                .header("access-control-allow-headers", &self.allowed_headers)
                .header("access-control-max-age", "86400")
                .body(crate::proxy::plugin::full_body(Bytes::new()))?;
            return Ok(Some(resp));
        }
        Ok(None)
    }

    async fn post_process(
        &self,
        mut resp: ProxyResponse,
        _ctx: &RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        let headers = resp.headers_mut();
        if !headers.contains_key("access-control-allow-origin") {
            headers.insert(
                "access-control-allow-origin",
                self.allowed_origins.parse().unwrap(),
            );
        }
        if !headers.contains_key("access-control-allow-methods") {
            headers.insert(
                "access-control-allow-methods",
                self.allowed_methods.parse().unwrap(),
            );
        }
        if !headers.contains_key("access-control-allow-headers") {
            headers.insert(
                "access-control-allow-headers",
                self.allowed_headers.parse().unwrap(),
            );
        }
        Ok(resp)
    }
}
