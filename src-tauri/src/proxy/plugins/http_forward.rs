use async_trait::async_trait;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response, StatusCode};
use regex::Regex;
use std::collections::HashMap;

use crate::proxy::plugin::{ProxyHandler, ProxyResponse, RequestContext, ResponseBody};
use crate::proxy::rewriter::{classify_content, detect_charset, rewrite_urls, RewriteTarget};
use crate::proxy::{ActiveOrigin, ProxyKeyMap};

pub struct HttpForwardPlugin {
    client: reqwest::Client,
    key_map: ProxyKeyMap,
    active_origin: ActiveOrigin,
}

impl HttpForwardPlugin {
    pub fn new(
        key_map: ProxyKeyMap,
        active_origin: ActiveOrigin,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self { client, key_map, active_origin })
    }

    fn build_proxy_prefix(host: &str) -> String {
        format!("http://{}", host)
    }

    fn resolve_relative_path(original_url: &str, request_path: &str) -> String {
        let base_dir = match original_url.rfind('/') {
            Some(pos) => original_url[..=pos].to_string(),
            None => format!("{}/", original_url),
        };
        let clean_path = request_path.trim_start_matches('/');
        format!("{}{}", base_dir, clean_path)
    }
}

#[async_trait]
impl ProxyHandler for HttpForwardPlugin {
    fn name(&self) -> &'static str {
        "http_forward"
    }

    fn can_handle(&self, req: &Request<Incoming>) -> bool {
        let path = req.uri().path();

        // __proxy__{key} 路径 → 新格式
        if path.starts_with("/__proxy__") {
            return true;
        }

        // ?url= 查询参数（向后兼容）
        let query = req.uri().query().unwrap_or("");
        let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).collect();
        if params.contains_key("url") {
            return true;
        }

        if req.method() == Method::GET {
            if let Some(referer) = req.headers().get("referer") {
                if let Ok(referer_str) = referer.to_str() {
                    if referer_str.contains("/proxy?url=") || referer_str.contains("/__proxy__") {
                        return true;
                    }
                }
            }
            // Fallthrough: catch subresource requests that arrive without ?url= or a useful Referer
            let path = req.uri().path();
            if path != "/proxy" && path != "/" {
                return true;
            }
        }
        false
    }

    async fn handle(
        &self,
        req: Request<Incoming>,
        ctx: RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        let path = req.uri().path().to_string();

        // 路由 ①：__proxy__{key} 路径
        if let Some(rest) = path.strip_prefix("/__proxy__") {
            return self.handle_proxy_request(req, rest, ctx).await;
        }

        // 路由 ②：?url= 向后兼容
        let query = req.uri().query().unwrap_or("").to_string();
        let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).collect();
        if let Some(target_url) = params.get("url") {
            *self.active_origin.write().await = Some(target_url.clone());
            return self.forward(req, target_url, target_url, "", &ctx).await;
        }

        // 路由 ③：fallthrough → Referer → active_origin
        let target = self.resolve_target(&req).await?;
        self.forward(req, &target, &target, "", &ctx).await
    }
}

impl HttpForwardPlugin {
    async fn handle_proxy_request(
        &self,
        req: Request<Incoming>,
        rest: &str,
        ctx: RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        // 1. 提取 key
        let key_end = rest.find('/').or_else(|| rest.find('?')).unwrap_or(rest.len());
        let key = &rest[..key_end];

        // 2. 查询源地址
        let source_url = self.key_map.read().await.get(key).cloned()
            .ok_or_else(|| format!("Proxy key not found: {}", key))?;

        // 3. 写入 active_origin（兜底缓存）
        *self.active_origin.write().await = Some(source_url.clone());

        // 4. 构造转发目标 URL
        // 注：必须用 starts_with('?') 而非 find('?')，因为同域路径也可能含 ?（如 key/path?query）
        let forward_url = if rest[key_end..].starts_with('?') {
            // 跨域：__proxy__{key}?url=xxx
            let qs = &rest[key_end + 1..];
            let params: HashMap<_, _> = url::form_urlencoded::parse(qs.as_bytes()).collect();
            params.get("url").cloned().unwrap_or(source_url.clone())
        } else if let Some(spos) = rest[key_end..].find('/') {
            // 同域：__proxy__{key}/path
            let remaining = &rest[key_end + spos..];
            format!("{}{}", source_url.trim_end_matches('/'), remaining)
        } else {
            source_url.clone()
        };

        // 5. 转发（rewriter 传入 key）
        self.forward(req, &forward_url, &source_url, key, &ctx).await
    }

    async fn resolve_target(
        &self,
        req: &Request<Incoming>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // 优先 Referer
        if let Some(referer) = req.headers().get("referer").and_then(|v| v.to_str().ok()) {
            if let Some(pos) = referer.find("/proxy?url=") {
                let encoded = &referer[pos + "/proxy?url=".len()..];
                let decoded = urlencoding::decode(encoded)
                    .map_err(|e| format!("Failed to decode Referer URL: {}", e))?;

                let request_path = req.uri().path();
                let full_path = match req.uri().query() {
                    Some(query) => format!("{}?{}", request_path, query),
                    None => request_path.to_string(),
                };

                let upstream = Self::resolve_relative_path(&decoded, &full_path);
                if upstream.starts_with("http://") || upstream.starts_with("https://") {
                    return Ok(upstream);
                }
            }
            if let Some(pos) = referer.find("/__proxy__") {
                let href = &referer[pos..];
                let key_end = href.find('/').or_else(|| href.find('?')).unwrap_or(href.len());
                let key = &href[..key_end];
                if let Some(source) = self.key_map.read().await.get(key).cloned() {
                    let request_path = req.uri().path();
                    let upstream = Self::resolve_relative_path(&source, request_path);
                    if upstream.starts_with("http://") || upstream.starts_with("https://") {
                        return Ok(upstream);
                    }
                }
            }
        }

        // 回退 active_origin
        self.active_origin.read().await.clone()
            .ok_or_else(|| "No target found (no Referer and no active origin)".to_string().into())
    }

    async fn forward(
        &self,
        req: Request<Incoming>,
        forward_url: &str,
        source_url: &str,
        proxy_key: &str,
        ctx: &RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        let method = req.method().clone();
        let proxy_prefix_host = req
            .headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .map(|h| Self::build_proxy_prefix(h))
            .unwrap_or_default();

        let hop_by_hop = [
            "host",
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailers",
            "transfer-encoding",
            "upgrade",
        ];
        let mut headers_to_forward: Vec<(String, String)> = Vec::new();
        for (key, value) in req.headers() {
            let key_lower = key.as_str().to_lowercase();
            if !hop_by_hop.contains(&key_lower.as_str()) {
                if let Ok(v) = value.to_str() {
                    headers_to_forward.push((key.as_str().to_string(), v.to_string()));
                }
            }
        }

        let body_bytes = BodyExt::collect(req.into_body())
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();

        let mut req_builder = match method {
            Method::GET => self.client.get(forward_url),
            Method::POST => self.client.post(forward_url).body(body_bytes.clone()),
            Method::PUT => self.client.put(forward_url).body(body_bytes.clone()),
            Method::PATCH => self.client.patch(forward_url).body(body_bytes.clone()),
            Method::DELETE => self.client.delete(forward_url),
            Method::HEAD => self.client.head(forward_url),
            _ => self.client.get(forward_url),
        };

        for (key, value) in &headers_to_forward {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }

        match req_builder.send().await {
            Ok(upstream) => {
                let status = upstream.status();
                let upstream_headers = upstream.headers().clone();

                let mut builder = Response::builder().status(status);

                for (key, value) in &upstream_headers {
                    let key_lower = key.as_str().to_lowercase();

                    if key_lower == "content-length"
                        || key_lower == "transfer-encoding"
                        || key_lower == "content-encoding"
                    {
                        continue;
                    }

                    if key_lower == "content-type" {
                        if let Ok(val) = value.to_str() {
                            let re = Regex::new(r"charset=[^\s;]+").unwrap();
                            let new_val = re.replace(val, "charset=utf-8");
                            builder = builder.header(key, new_val.as_ref());
                        } else {
                            builder = builder.header(key, value.clone());
                        }
                        continue;
                    }

                    builder = builder.header(key, value.clone());
                }

                let body_bytes = upstream.bytes().await.unwrap_or_default();

                let content_type = upstream_headers
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();

                let target_url = forward_url.to_string();

                let body: ResponseBody = if ctx.should_rewrite && !proxy_prefix_host.is_empty() {
                    let target = classify_content(&content_type);
                    match target {
                        RewriteTarget::Html | RewriteTarget::Css | RewriteTarget::Js => {
                            let encoding = detect_charset(&content_type);
                            let (body_str, _, _) = encoding.decode(&body_bytes);
                            let proxy_key_owned = proxy_key.to_string();
                            let target_url_clone = target_url.clone();
                            let proxy_prefix_clone = proxy_prefix_host.clone();
                            let rewritten = std::panic::catch_unwind(
                                std::panic::AssertUnwindSafe(|| {
                                    rewrite_urls(
                                        &body_str,
                                        &target_url_clone,
                                        &proxy_prefix_clone,
                                        &proxy_key_owned,
                                        target,
                                    )
                                }),
                            );
                            match rewritten {
                                Ok(cow) => {
                                    let new_bytes = cow.as_bytes().to_vec();
                                    builder = builder
                                        .header("content-length", new_bytes.len().to_string());
                                    Full::new(Bytes::from(new_bytes))
                                }
                                Err(_) => {
                                    builder = builder
                                        .header("content-length", body_bytes.len().to_string());
                                    Full::new(body_bytes)
                                }
                            }
                        }
                        _ => {
                            builder = builder
                                .header("content-length", body_bytes.len().to_string());
                            Full::new(body_bytes)
                        }
                    }
                } else {
                    builder = builder
                        .header("content-length", body_bytes.len().to_string());
                    Full::new(body_bytes)
                };

                Ok(builder.body(body).unwrap())
            }
            Err(e) => Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("content-type", "text/plain; charset=utf-8")
                .body(Full::new(Bytes::from(format!(
                    "Proxy request failed: {}",
                    e
                ))))
                .unwrap()),
        }
    }
}
