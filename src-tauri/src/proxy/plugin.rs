use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response, StatusCode};
use hyper::header::{HeaderName, HeaderValue};
use reqwest;

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

/// HTTP 反向代理处理器 - 处理 /proxy 路径
/// 查询参数：url=目标URL
/// 功能：转发请求到目标 URL，剥离 CSP、X-Frame-Options 等安全头，支持 iframe 嵌入
pub struct HttpReverseProxyHandler {
    client: reqwest::Client,
}

impl HttpReverseProxyHandler {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create reqwest client: {}", e))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl ProxyHandler for HttpReverseProxyHandler {
    fn name(&self) -> &'static str {
        "http-reverse-proxy"
    }

    fn can_handle(&self, req: &Request<Incoming>) -> bool {
        req.uri().path().starts_with("/proxy")
    }

    async fn handle(
        &self,
        req: Request<Incoming>,
        _ctx: RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        // 提取目标 URL
        let uri = req.uri();
        let query = uri.query().unwrap_or("");
        let target_url = match url::form_urlencoded::parse(query.as_bytes())
            .find(|(k, _)| k == "url")
            .map(|(_, v)| v.into_owned())
        {
            Some(url) => url,
            None => {
                return Ok(Self::error_response(400, "Missing 'url' query parameter"));
            }
        };

        // 解析并验证目标 URL
        let target_url = match url::Url::parse(&target_url) {
            Ok(u) => u,
            Err(_) => {
                return Ok(Self::error_response(400, "Invalid target URL"));
            }
        };

        // 只允许 http/https
        if target_url.scheme() != "http" && target_url.scheme() != "https" {
            return Ok(Self::error_response(400, "Only http/https URLs are allowed"));
        }

        // 构建请求
        let method = req.method().clone();
        let mut proxy_req = self.client.request(method.clone(), target_url.as_str());

        // 转发头部（过滤 hop-by-hop 头）
        let hop_by_hop = [
            "host", "connection", "keep-alive", "proxy-authenticate",
            "proxy-authorization", "te", "trailers", "transfer-encoding", "upgrade",
            "content-length",
        ];

        for (name, value) in req.headers() {
            let name_str = name.as_str().to_lowercase();
            if !hop_by_hop.contains(&name_str.as_str()) {
                if let Ok(v) = value.to_str() {
                    proxy_req = proxy_req.header(name.as_str(), v);
                }
            }
        }

        // 获取请求体
        let body_bytes = req
            .into_body()
            .collect()
            .await
            .map_err(|e| format!("读取请求体失败: {}", e))?
            .to_bytes();
        if !body_bytes.is_empty() {
            proxy_req = proxy_req.body(body_bytes.to_vec());
        }

        // 发送请求
        let upstream_resp = proxy_req.send().await
            .map_err(|e| format!("Upstream request failed: {}", e))?;

        // 构建响应
        let status = upstream_resp.status();
        let mut resp_builder = Response::builder().status(status);

        // 剥离安全头
        let headers_to_strip = [
            "content-security-policy",
            "x-frame-options",
            "x-content-type-options",
            "referrer-policy",
            "permissions-policy",
            "cross-origin-embedder-policy",
            "cross-origin-opener-policy",
            "cross-origin-resource-policy",
        ];

        for (name, value) in upstream_resp.headers() {
            let name_str = name.as_str().to_lowercase();
            if !headers_to_strip.contains(&name_str.as_str()) {
                if let Ok(v) = value.to_str() {
                    if let Ok(header_name) = HeaderName::from_bytes(name.as_str().as_bytes()) {
                        if let Ok(header_value) = HeaderValue::from_str(v) {
                            resp_builder = resp_builder.header(header_name, header_value);
                        }
                    }
                }
            }
        }

        // 添加允许 iframe 的头
        resp_builder = resp_builder
            .header("access-control-allow-origin", "*")
            .header("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS")
            .header("access-control-allow-headers", "*")
            .header("x-frame-options", "ALLOWALL");

        let body_bytes = upstream_resp.bytes().await
            .map_err(|e| format!("Failed to read upstream body: {}", e))?;

        let resp = resp_builder
            .body(Full::new(Bytes::from(body_bytes)))
            .map_err(|e| format!("Failed to build response: {}", e))?;

        Ok(resp)
    }
}

impl HttpReverseProxyHandler {
    fn error_response(status: u16, msg: &str) -> ProxyResponse {
        Response::builder()
            .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
            .header("content-type", "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from(msg.to_string())))
            .unwrap()
    }
}
