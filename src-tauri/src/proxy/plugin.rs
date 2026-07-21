use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response, StatusCode};

pub type ResponseBody = Full<Bytes>;
pub type ProxyResponse = Response<ResponseBody>;

#[derive(Clone)]
pub struct RequestContext {
    pub target_url: Option<String>,
    pub should_rewrite: bool,
    pub start_time: Instant,
    pub plugin_data: HashMap<String, String>,
}

impl RequestContext {
    pub fn new() -> Self {
        Self {
            target_url: None,
            should_rewrite: true,
            start_time: Instant::now(),
            plugin_data: HashMap::new(),
        }
    }
}

#[async_trait]
pub trait ProxyPlugin: Send + Sync {
    fn name(&self) -> &'static str;

    fn priority(&self) -> i32 {
        0
    }

    async fn pre_process(
        &self,
        _req: &mut Request<Incoming>,
        _ctx: &mut RequestContext,
    ) -> Result<Option<ProxyResponse>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(None)
    }

    async fn post_process(
        &self,
        resp: ProxyResponse,
        _ctx: &RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        Ok(resp)
    }
}

#[async_trait]
pub trait ProxyHandler: Send + Sync {
    fn name(&self) -> &'static str;

    fn can_handle(&self, req: &Request<Incoming>) -> bool;

    async fn handle(
        &self,
        req: Request<Incoming>,
        ctx: RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct PluginChain {
    plugins: Vec<Arc<dyn ProxyPlugin>>,
    handlers: Vec<Arc<dyn ProxyHandler>>,
}

impl PluginChain {
    pub fn new(
        mut plugins: Vec<Arc<dyn ProxyPlugin>>,
        handlers: Vec<Arc<dyn ProxyHandler>>,
    ) -> Self {
        plugins.sort_by_key(|p| p.priority());
        Self { plugins, handlers }
    }

    pub fn handlers(&self) -> &[Arc<dyn ProxyHandler>] {
        &self.handlers
    }

    fn find_handler(&self, req: &Request<Incoming>) -> Option<&dyn ProxyHandler> {
        self.handlers
            .iter()
            .find(|h| h.can_handle(req))
            .map(|h| h.as_ref())
    }

    pub async fn process(
        &self,
        mut req: Request<Incoming>,
        mut ctx: RequestContext,
    ) -> ProxyResponse {
        // Phase 1: pre-process (request → all middleware)
        for plugin in &self.plugins {
            match plugin.pre_process(&mut req, &mut ctx).await {
                Ok(Some(resp)) => return resp,
                Ok(None) => {}
                Err(e) => {
                    return Self::error_response(
                        502,
                        &format!("[{}] pre_process error: {}", plugin.name(), e),
                    )
                }
            }
        }

        // Phase 2: handler
        let handler = self.find_handler(&req);
        let mut resp = match handler {
            Some(h) => match h.handle(req, ctx.clone()).await {
                Ok(r) => r,
                Err(e) => {
                    return Self::error_response(
                        502,
                        &format!("[{}] handler error: {}", h.name(), e),
                    )
                }
            },
            None => Self::error_response(404, "No handler for this request"),
        };

        // Phase 3: post-process (response ← all middleware, reversed)
        for plugin in self.plugins.iter().rev() {
            match plugin.post_process(resp, &ctx).await {
                Ok(r) => resp = r,
                Err(e) => {
                    return Self::error_response(
                        502,
                        &format!("[{}] post_process error: {}", plugin.name(), e),
                    )
                }
            }
        }

        resp
    }

    fn error_response(status: u16, msg: &str) -> ProxyResponse {
        Response::builder()
            .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
            .header("content-type", "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from(msg.to_string())))
            .unwrap()
    }
}
