use async_trait::async_trait;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response, StatusCode};
use regex::Regex;
use std::collections::HashMap;

use crate::proxy::plugin::{ProxyHandler, ProxyResponse, RequestContext, ResponseBody};
use crate::proxy::rewriter::{classify_content, detect_charset, rewrite_urls, RewriteTarget};

pub struct HttpForwardPlugin {
    client: reqwest::Client,
}

impl HttpForwardPlugin {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self { client })
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

    fn resolve_url_from_referer(req: &Request<Incoming>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let referer = req.headers()
            .get("referer")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| "Missing Referer header".to_string())?;

        let pos = referer.find("/proxy?url=")
            .ok_or_else(|| "Referer does not contain proxy URL".to_string())?;

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
            Ok(upstream)
        } else {
            Err("Resolved URL is not absolute".to_string())
        }
    }
}

#[async_trait]
impl ProxyHandler for HttpForwardPlugin {
    fn name(&self) -> &'static str {
        "http_forward"
    }

    fn can_handle(&self, req: &Request<Incoming>) -> bool {
        let query = req.uri().query().unwrap_or("");
        let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).collect();
        if params.contains_key("url") {
            return true;
        }
        if req.method() == Method::GET {
            if let Some(referer) = req.headers().get("referer") {
                if let Ok(referer_str) = referer.to_str() {
                    if referer_str.contains("/proxy?url=") {
                        return true;
                    }
                }
            }
            // Fallthrough: catch subresource requests (e.g. GET /noise-c.wasm)
            // that arrive without ?url= or a useful Referer.
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
        let query = req.uri().query().unwrap_or("");
        let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).collect();
        let target_url = match params.get("url") {
            Some(url) => url.to_string(),
            None => Self::resolve_url_from_referer(&req)?,
        };

        // Extract all data from req before consuming it
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

        let forward_url = &target_url;

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

                    // Structural headers: skip, fresh framing computed by hyper
                    if key_lower == "content-length"
                        || key_lower == "transfer-encoding"
                        || key_lower == "content-encoding"
                    {
                        continue;
                    }

                    // Force charset to UTF-8
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

                // URL rewriting for HTML/CSS content
                let content_type = upstream_headers
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();

                let body: ResponseBody = if ctx.should_rewrite && !proxy_prefix_host.is_empty() {
                    let target = classify_content(&content_type);
                    match target {
                        RewriteTarget::Html | RewriteTarget::Css | RewriteTarget::Js => {
                            let encoding = detect_charset(&content_type);
                            let (body_str, _, _) = encoding.decode(&body_bytes);
                            let rewritten = std::panic::catch_unwind(
                                std::panic::AssertUnwindSafe(|| {
                                    rewrite_urls(&body_str, &target_url, &proxy_prefix_host, target)
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
