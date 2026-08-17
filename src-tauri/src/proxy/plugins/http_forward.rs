use async_trait::async_trait;
use http_body_util::BodyExt;
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response, StatusCode};
use regex::Regex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// 代理请求序号（排查日志关联：同一请求的所有日志带相同 req#）
static REQ_SEQ: AtomicU64 = AtomicU64::new(0);

use crate::proxy::hometier_protocol::{
    cookie_jars, inject_local_http_script, looks_like_html_body, now_epoch, persist_cookie_to_db,
    relax_csp, PerHostCookieJar,
};
use crate::proxy::plugin::{
    full_body, stream_body, ProxyHandler, ProxyResponse, RequestContext, ResponseBody,
};
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
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10));
        for cert in crate::proxy::proxy_ca_certs() {
            builder = builder.add_root_certificate(cert);
        }
        let client = builder.build()?;
        crate::log_info!(
            "[DBG-PROXY] reqwest 客户端构建完成: features=gzip+brotli (未显式设置 Accept-Encoding 时自动注入 'gzip, br', 自动解压并剥除 content-encoding)"
        );
        Ok(Self { client, key_map, active_origin })
    }

    fn rid_of(ctx: &RequestContext) -> String {
        ctx.plugin_data.get("req_id").cloned().unwrap_or_default()
    }

    fn hdr_str(headers: &hyper::header::HeaderMap, name: &str) -> String {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    }

    fn cookie_name_list(v: &str) -> String {
        v.split(';')
            .filter_map(|p| p.split('=').next())
            .map(|n| n.trim())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// 出站头打印（Cookie 值打码：仅列 name）
    fn masked_headers(list: &[(String, String)]) -> String {
        list.iter()
            .map(|(k, v)| {
                if k.eq_ignore_ascii_case("cookie") {
                    format!("{}:[{}]", k, Self::cookie_name_list(v))
                } else {
                    format!("{}={}", k, v)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn build_proxy_prefix(host: &str) -> String {
        format!("http://{}", host)
    }

    /// 将请求路径相对 original_url 解析为上游 URL：
    /// - 根相对路径（/ 开头）：按 HTML 规范基于 origin（scheme://host[:port]）解析
    /// - 相对路径：基于文档目录（原 base_dir 逻辑）
    fn resolve_relative_path(original_url: &str, request_path: &str) -> String {
        let clean_path = request_path.trim_start_matches('/');
        if request_path.starts_with('/') {
            let scheme = if original_url.starts_with("https://") {
                "https"
            } else {
                "http"
            };
            let origin = match original_url.split("://").nth(1) {
                Some(rest) => match rest.find('/') {
                    Some(pos) => format!("{}://{}", scheme, &rest[..pos]),
                    None => format!("{}://{}", scheme, rest),
                },
                None => original_url.to_string(),
            };
            return format!("{}/{}", origin.trim_end_matches('/'), clean_path);
        }
        let base_dir = match original_url.rfind('/') {
            Some(pos) => original_url[..=pos].to_string(),
            None => format!("{}/", original_url),
        };
        format!("{}{}", base_dir, clean_path)
    }

    /// 从 URL 提取 upstream origin（host[:port]）与请求路径
    fn split_upstream(url: &str) -> (String, String) {
        let rest = url
            .split("://")
            .nth(1)
            .unwrap_or(url);
        match rest.find('/') {
            Some(pos) => (rest[..pos].to_string(), rest[pos..].split('?').next().unwrap_or("/").to_string()),
            None => (rest.to_string(), "/".to_string()),
        }
    }

    /// 改写 3xx Location 头：将上游地址/绝对路径改回代理地址，避免跳转逃逸代理
    fn rewrite_location(loc: &str, forward_url: &str, key: &str, prefix: &str) -> Option<String> {
        if key.is_empty() || prefix.is_empty() {
            return None;
        }
        if loc.starts_with('/') {
            return Some(format!("{}/__proxy__{}{}", prefix, key, loc));
        }
        let (origin, _) = Self::split_upstream(forward_url);
        if origin.is_empty() {
            return None;
        }
        let scheme = if forward_url.starts_with("https://") {
            "https"
        } else {
            "http"
        };
        let base = format!("{}://{}", scheme, origin);
        if let Some(rest) = loc.strip_prefix(&base) {
            return Some(format!("{}/__proxy__{}{}", prefix, key, rest));
        }
        None
    }

    /// 合并浏览器原生 Cookie 与 jar Cookie（按 name 去重，jar 优先）
    fn merge_cookie_headers(native: Option<&str>, jar: &str) -> String {
        let mut pairs: Vec<(String, String)> = Vec::new();
        if let Some(n) = native {
            for part in n.split(';') {
                if let Some(eq) = part.find('=') {
                    pairs.push((
                        part[..eq].trim().to_string(),
                        part[eq + 1..].trim().to_string(),
                    ));
                }
            }
        }
        for part in jar.split(';') {
            if let Some(eq) = part.find('=') {
                let name = part[..eq].trim();
                pairs.retain(|(k, _)| k != name);
                pairs.push((name.to_string(), part[eq + 1..].trim().to_string()));
            }
        }
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// 从 Content-Disposition / URL 提取下载文件名
    fn extract_filename(cd: &str, url: &str) -> String {
        let lower = cd.to_lowercase();
        if let Some(pos) = lower.find("filename*=") {
            let rest = &cd[pos + "filename*=".len()..];
            let val = rest.split(';').next().unwrap_or("").trim().trim_matches('"');
            let decoded = urlencoding::decode(val)
                .map(|s| s.into_owned())
                .unwrap_or_else(|_| val.to_string());
            if let Some(eq) = decoded.find("''") {
                return decoded[eq + 2..].to_string();
            }
            return decoded;
        }
        if let Some(pos) = lower.find("filename=") {
            let rest = &cd[pos + "filename=".len()..];
            let val = rest.split(';').next().unwrap_or("").trim().trim_matches('"');
            if !val.is_empty() {
                return val.to_string();
            }
        }
        let trimmed = url.trim_end_matches('/');
        trimmed
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("download")
            .to_string()
    }

    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| if c == '/' || c == '\\' { '_' } else { c })
            .collect()
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
        mut ctx: RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        ctx.plugin_data
            .insert("req_id".into(), REQ_SEQ.fetch_add(1, Ordering::Relaxed).to_string());
        let rid = Self::rid_of(&ctx);
        let path = req.uri().path().to_string();
        let path_query = match req.uri().query() {
            Some(q) => format!("{}?{}", path, q),
            None => path.clone(),
        };
        let cookie_raw = Self::hdr_str(req.headers(), "cookie");
        let cookie_list = Self::cookie_name_list(&cookie_raw);
        crate::log_info!(format!(
            "[DBG-PROXY][req#{}] 入站 {} {} | CT={} | Cookie=[{}] | INM={} IMS={} | AE={} | UA={}",
            rid,
            req.method(),
            path_query,
            Self::hdr_str(req.headers(), "content-type"),
            if cookie_raw.is_empty() { "无" } else { &cookie_list },
            Self::hdr_str(req.headers(), "if-none-match"),
            Self::hdr_str(req.headers(), "if-modified-since"),
            Self::hdr_str(req.headers(), "accept-encoding"),
            Self::hdr_str(req.headers(), "user-agent"),
        ));

        // 路由 ①：__proxy__{key} 路径
        if let Some(rest) = path.strip_prefix("/__proxy__") {
            let key_end = rest.find('/').or_else(|| rest.find('?')).unwrap_or(rest.len());
            crate::log_info!(format!(
                "[DBG-PROXY][req#{}] 路由① __proxy__ key={} rest={}",
                rid, &rest[..key_end], rest
            ));
            return self.handle_proxy_request(req, rest, ctx).await;
        }

        // 路由 ②：?url= 向后兼容
        let query = req.uri().query().unwrap_or("").to_string();
        let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).collect();
        if let Some(target_url) = params.get("url") {
            let target_url = target_url.to_string();
            crate::log_info!(format!(
                "[DBG-PROXY][req#{}] 路由② ?url= target={}",
                rid, target_url
            ));
            *self.active_origin.write().await = Some(target_url.clone());
            return self.forward(req, &target_url, &target_url, "", &ctx).await;
        }

        // 路由 ③：fallthrough → 直通模式（无代理转换，替换 proxy 地址为源地址直接请求）
        let target = match self.resolve_target(&req).await {
            Ok(t) => t,
            Err(e) => {
                crate::log_error!(format!("直通模式: 无法解析上游目标: {}", e));
                return Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header("content-type", "text/plain; charset=utf-8")
                    .body(full_body(Bytes::from(format!("no upstream target: {}", e))))
                    .unwrap());
            }
        };
        *self.active_origin.write().await = Some(target.clone());
        crate::log_info!(format!(
            "[DBG-PROXY][req#{}] 路由③ 直通 target={}",
            rid, target
        ));
        self.passthrough(req, &target, &ctx).await
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
        let rid = Self::rid_of(&ctx);
        let forward_url: String = if rest[key_end..].starts_with('?') {
            // 跨域：__proxy__{key}?url=xxx
            let qs = &rest[key_end + 1..];
            let params: HashMap<_, _> = url::form_urlencoded::parse(qs.as_bytes()).collect();
            let u = params.get("url").map(|v| v.to_string()).unwrap_or(source_url.clone());
            crate::log_info!(format!(
                "[DBG-PROXY][req#{}] 跨域转发: ?url={} (回退 source={})",
                rid, u, source_url
            ));
            u
        } else if let Some(spos) = rest[key_end..].find('/') {
            // 同域：__proxy__{key}/path[?query]
            // remaining 可能已含 query（请求行 query 与 req.uri().query() 重复），先拆路径再拼一次 query
            let remaining = &rest[key_end + spos..];
            let path_part = remaining.split('?').next().unwrap_or(remaining);
            let query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();
            let u = format!("{}{}{}", source_url.trim_end_matches('/'), path_part, query);
            crate::log_info!(format!(
                "[DBG-PROXY][req#{}] 同域转发: source={} path={} query={} → {}",
                rid, source_url, path_part, query, u
            ));
            u
        } else {
            crate::log_info!(format!(
                "[DBG-PROXY][req#{}] 无路径回退: source={}",
                rid, source_url
            ));
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
                let rest = &referer[pos + "/__proxy__".len()..];
                let key_end = rest.find('/').or_else(|| rest.find('?')).unwrap_or(rest.len());
                let key = &rest[..key_end];
                if let Some(source) = self.key_map.read().await.get(key).cloned() {
                    let request_path = req.uri().path();
                    let full_path = match req.uri().query() {
                        Some(query) => format!("{}?{}", request_path, query),
                        None => request_path.to_string(),
                    };
                    let upstream = Self::resolve_relative_path(&source, &full_path);
                    if upstream.starts_with("http://") || upstream.starts_with("https://") {
                        return Ok(upstream);
                    }
                }
            }
        }

        // 回退 active_origin：无 Referer（字体/图片等跨域或原生请求）时，
        // 用请求路径相对 active_origin 解析，避免返回裸 origin 导致目标路径丢失
        if let Some(origin) = self.active_origin.read().await.clone() {
            let request_path = req.uri().path();
            let full_path = match req.uri().query() {
                Some(query) => format!("{}?{}", request_path, query),
                None => request_path.to_string(),
            };
            let upstream = Self::resolve_relative_path(&origin, &full_path);
            if upstream.starts_with("http://") || upstream.starts_with("https://") {
                return Ok(upstream);
            }
            return Ok(origin);
        }
        Err("No target found (no Referer and no active origin)".to_string().into())
    }

    async fn passthrough(
        &self,
        req: Request<Incoming>,
        target_url: &str,
        ctx: &RequestContext,
    ) -> Result<ProxyResponse, Box<dyn std::error::Error + Send + Sync>> {
        let method = req.method().clone();
        let req_path = req.uri().path().to_lowercase();
        let rid = Self::rid_of(ctx);
        crate::log_debug!(format!(
            "[DBG-PROXY][req#{}] 直通出站 {} {} | 头 {} 项: {}",
            rid,
            method,
            target_url,
            req.headers().len(),
            "见入站摘要 (直通不改造请求头)"
        ));

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
            if !hop_by_hop.contains(&key_lower.as_str()) && key_lower != "content-length" {
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
            Method::GET => self.client.get(target_url),
            Method::POST => self.client.post(target_url).body(body_bytes.clone()),
            Method::PUT => self.client.put(target_url).body(body_bytes.clone()),
            Method::PATCH => self.client.patch(target_url).body(body_bytes.clone()),
            Method::DELETE => self.client.delete(target_url),
            Method::HEAD => self.client.head(target_url),
            Method::OPTIONS => self.client.request(Method::OPTIONS, target_url),
            _ => self.client.get(target_url),
        };

        for (key, value) in &headers_to_forward {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }

        // 移动仿真：伪装移动端 UA
        if crate::proxy::hometier_protocol::device_mode() == "mobile" {
            req_builder = req_builder.header(
                "user-agent",
                crate::proxy::hometier_protocol::MOBILE_UA,
            );
        }

        match req_builder.send().await {
            Ok(upstream) => {
                let status = upstream.status();
                let rid = Self::rid_of(ctx);
                crate::log_info!(format!(
                    "[DBG-PROXY][req#{}] 直通上游 {status} {} | CT={} CL={} CE={}",
                    rid,
                    target_url,
                    Self::hdr_str(upstream.headers(), "content-type"),
                    Self::hdr_str(upstream.headers(), "content-length"),
                    Self::hdr_str(upstream.headers(), "content-encoding"),
                ));
                if status.as_u16() >= 400 {
                    crate::log_warn!(format!(
                        "[DBG-PROXY][req#{}] 直通上游响应异常状态 {status} for {}",
                        rid, target_url
                    ));
                }
                let mut builder = Response::builder().status(status);

                for (key, value) in upstream.headers() {
                    let key_lower = key.as_str().to_lowercase();
                    if key_lower == "content-length"
                        || key_lower == "transfer-encoding"
                        || key_lower == "content-encoding"
                    {
                        continue;
                    }
                    builder = builder.header(key, value.clone());
                }

                if status == StatusCode::NOT_MODIFIED || status == StatusCode::NO_CONTENT {
                    return Ok(builder.body(full_body(Bytes::new())).unwrap());
                }
                // 静态资源（.css/.js/.mjs 及字体/图片）若上游返回 HTML，说明目标解析错误（实际为 404 页等），
                // 浏览器 strict-mode 会拒绝并报「非 CSS MIME 类型」错误 / 图标字体失效（□□）；改为返回 502 + text/plain，
                // 保留原始 content-type 到日志，避免触发页面级阻断。
                let is_static_asset = req_path.ends_with(".css")
                    || req_path.ends_with(".js")
                    || req_path.ends_with(".mjs")
                    || req_path.ends_with(".woff")
                    || req_path.ends_with(".woff2")
                    || req_path.ends_with(".ttf")
                    || req_path.ends_with(".eot")
                    || req_path.ends_with(".svg")
                    || req_path.ends_with(".png")
                    || req_path.ends_with(".jpg")
                    || req_path.ends_with(".jpeg")
                    || req_path.ends_with(".gif")
                    || req_path.ends_with(".webp")
                    || req_path.ends_with(".ico");
                if is_static_asset {
                    if let Some(ct) = upstream
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                    {
                        if ct.contains("text/html") {
                            crate::log_error!(format!(
                                "直通模式: 静态资源 {}, 上游返回 HTML (status={}, ct={}), 改返 502",
                                req_path, status.as_u16(), ct
                            ));
                            return Ok(Response::builder()
                                .status(StatusCode::BAD_GATEWAY)
                                .header("content-type", "text/plain; charset=utf-8")
                                .body(full_body(Bytes::from(format!(
                                    "Upstream returned HTML for static asset {} (status {})",
                                    req_path, status.as_u16()
                                ))))
                                .unwrap());
                        }
                    }
                }
                Ok(builder.body(stream_body(upstream.bytes_stream())).unwrap())
            }
            Err(e) => {
                crate::log_error!(format!(
                    "[DBG-PROXY][req#{}] 直通上游请求失败 {} {}",
                    Self::rid_of(ctx),
                    target_url,
                    e
                ));
                Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .header("content-type", "text/plain; charset=utf-8")
                    .body(full_body(Bytes::from(format!(
                        "Passthrough request failed: {}",
                        e
                    ))))
                    .unwrap())
            }
        }
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
            if !hop_by_hop.contains(&key_lower.as_str()) && key_lower != "content-length" {
                if let Ok(v) = value.to_str() {
                    headers_to_forward.push((key.as_str().to_string(), v.to_string()));
                }
            }
        }
        crate::log_debug!(format!(
            "[DBG-PROXY][req#{}] 出站头(合并 jar 前, {} 项): {}",
            Self::rid_of(ctx),
            headers_to_forward.len(),
            Self::masked_headers(&headers_to_forward)
        ));

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
            Method::OPTIONS => self.client.request(Method::OPTIONS, forward_url),
            _ => self.client.get(forward_url),
        };

        for (key, value) in &headers_to_forward {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }

        // 移动仿真：伪装移动端 UA
        if crate::proxy::hometier_protocol::device_mode() == "mobile" {
            req_builder = req_builder.header(
                "user-agent",
                crate::proxy::hometier_protocol::MOBILE_UA,
            );
        }

        // 合并 jar Cookie（WebView 原生无法持有的 Secure/HttpOnly/上游域 Cookie）
        {
            let (upstream_origin, upstream_path) = Self::split_upstream(forward_url);
            let rid = Self::rid_of(ctx);
            if !upstream_origin.is_empty() {
                let jar_cookie = {
                    let mut jars = cookie_jars().lock().unwrap();
                    let jar = jars
                        .entry(upstream_origin.clone())
                        .or_insert_with(PerHostCookieJar::new);
                    jar.build_cookie_header(&upstream_origin, &upstream_path)
                };
                if let Some(jc) = jar_cookie {
                    let native_owned = headers_to_forward
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("cookie"))
                        .map(|(_, v)| v.clone());
                    let merged = Self::merge_cookie_headers(native_owned.as_deref(), &jc);
                    headers_to_forward
                        .retain(|(k, _)| !k.eq_ignore_ascii_case("cookie"));
                    headers_to_forward.push(("cookie".to_string(), merged.clone()));
                    crate::log_info!(format!(
                        "[DBG-PROXY][req#{}] jar Cookie 注入: origin={} native={} jar=[{}] → cookie=[{}]",
                        rid,
                        upstream_origin,
                        if native_owned.is_some() { "有" } else { "无" },
                        Self::cookie_name_list(&jc),
                        Self::cookie_name_list(&merged),
                    ));
                } else {
                    crate::log_info!(format!(
                        "[DBG-PROXY][req#{}] jar Cookie: 无匹配(origin={}) 不注入",
                        rid, upstream_origin
                    ));
                }
            }
        }
        crate::log_debug!(format!(
            "[DBG-PROXY][req#{}] 出站头(合并 jar 后, {} 项): {}",
            Self::rid_of(ctx),
            headers_to_forward.len(),
            Self::masked_headers(&headers_to_forward)
        ));
        for (key, value) in &headers_to_forward {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }

        let send_started = std::time::Instant::now();
        match req_builder.send().await {
            Ok(upstream) => {
                let status = upstream.status();
                let elapsed_ms = send_started.elapsed().as_millis();
                let upstream_headers = upstream.headers().clone();
                let rid = Self::rid_of(ctx);
                crate::log_info!(format!(
                    "[DBG-PROXY][req#{}] 上游 {status} {} 耗时{elapsed_ms}ms | CT={} CL={} CE={} ETag={} Vary={} xreq={} Set-Cookie=[{}]",
                    rid,
                    forward_url,
                    Self::hdr_str(upstream.headers(), "content-type"),
                    Self::hdr_str(upstream.headers(), "content-length"),
                    Self::hdr_str(upstream.headers(), "content-encoding"),
                    Self::hdr_str(upstream.headers(), "etag"),
                    Self::hdr_str(upstream.headers(), "vary"),
                    Self::hdr_str(upstream.headers(), "x-request-error"),
                    upstream_headers
                        .get_all("set-cookie")
                        .iter()
                        .filter_map(|v| v.to_str().ok())
                        .map(Self::cookie_name_list)
                        .collect::<Vec<_>>()
                        .join(";"),
                ));
                if status.as_u16() >= 400 {
                    crate::log_warn!(format!(
                        "[DBG-PROXY][req#{}] 上游响应异常状态 {status} for {}",
                        rid, forward_url
                    ));
                }

                // 捕获上游 Set-Cookie → 全局 jar + DB（登录态落库持久）
                let (upstream_origin, _) = Self::split_upstream(forward_url);
                if !upstream_origin.is_empty() {
                    for value in upstream_headers.get_all("set-cookie") {
                        if let Ok(val) = value.to_str() {
                            let stored = {
                                let mut jars = cookie_jars().lock().unwrap();
                                let jar = jars
                                    .entry(upstream_origin.clone())
                                    .or_insert_with(PerHostCookieJar::new);
                                jar.add_set_cookie(val)
                            };
                            if let Some(cookie) = stored {
                                persist_cookie_to_db(&upstream_origin, &cookie, now_epoch());
                            }
                        }
                    }
                }

                let mut builder = Response::builder().status(status);

                for (key, value) in &upstream_headers {
                    let key_lower = key.as_str().to_lowercase();

                    if key_lower == "content-length"
                        || key_lower == "transfer-encoding"
                        || key_lower == "content-encoding"
                        || key_lower == "x-content-type-options"
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

                    if key_lower == "location" {
                        if let Ok(val) = value.to_str() {
                            if let Some(new_loc) =
                                Self::rewrite_location(val, forward_url, proxy_key, &proxy_prefix_host)
                            {
                                builder = builder.header(key, new_loc.as_str());
                                continue;
                            }
                        }
                    }

                    builder = builder.header(key, value.clone());
                }

                let forward_url_lower = forward_url.to_lowercase();
                let is_cgi_or_php = forward_url_lower.contains(".cgi") || forward_url_lower.contains(".php");
                let content_type = upstream_headers
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();
                let is_sse = content_type.contains("text/event-stream");

                let target = classify_content(&content_type);
                let needs_rewrite = ctx.should_rewrite
                    && !proxy_prefix_host.is_empty()
                    && matches!(target, RewriteTarget::Html | RewriteTarget::Css | RewriteTarget::Js)
                    && !is_sse
                    && status != StatusCode::NOT_MODIFIED
                    && !status.is_redirection();
                crate::log_debug!(format!(
                    "[DBG-PROXY][req#{}] rewrite 决策: status={} target={:?} should_rewrite={} prefix='{}' is_sse={} → {}",
                    Self::rid_of(ctx),
                    status.as_u16(),
                    target,
                    ctx.should_rewrite,
                    proxy_prefix_host,
                    is_sse,
                    if needs_rewrite { "重写" } else { "跳过" }
                ));

                let mut csp_override: Option<String> = None;

                let is_html_target = matches!(target, RewriteTarget::Html);

                // 下载拦截：Content-Disposition: attachment → 落盘 downloads 目录 + 通知前端
                let content_disposition = upstream_headers
                    .get("content-disposition")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.to_string());
                let is_download = content_disposition
                    .as_deref()
                    .map(|cd| cd.to_lowercase().contains("attachment"))
                    .unwrap_or(false);

                let body: ResponseBody = if needs_rewrite {
                    let body_bytes = upstream.bytes().await.unwrap_or_default();
                    if status.as_u16() >= 400 {
                        let preview =
                            String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(512)]);
                        crate::log_warn!(format!(
                            "[DBG-PROXY][req#{}] 上游 {status} 响应体前512B: {}",
                            Self::rid_of(ctx),
                            preview.replace('\n', "\\n")
                        ));
                    }

                    // CGI/PHP 脚本 MIME 强制覆盖
                    if is_cgi_or_php && content_type.contains("text/html") {
                        let looks_like_html = body_bytes.starts_with(b"<") || body_bytes.starts_with(b"<!");
                        if !looks_like_html {
                            builder = builder.header("content-type", "application/javascript; charset=utf-8");
                        }
                    }

                    let target_url = forward_url.to_string();
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
                            let mut new_bytes = cow.as_bytes().to_vec();
                            crate::log_debug!(format!(
                                "[DBG-PROXY][req#{}] rewrite 完成: {}B → {}B",
                                Self::rid_of(ctx),
                                body_bytes.len(),
                                new_bytes.len()
                            ));
                            // HTML 页面注入本地代理脚本（WS 重写 + 移动仿真）并放宽 CSP
                            let mut csp_hash = String::new();
                            if is_html_target && looks_like_html_body(&new_bytes) {
                                let is_mobile =
                                    crate::proxy::hometier_protocol::device_mode() == "mobile";
                                let (injected, hash) =
                                    inject_local_http_script(new_bytes, is_mobile, proxy_key, source_url);
                                new_bytes = injected;
                                csp_hash = hash;
                            }
                            if !csp_hash.is_empty() {
                                let proxy_port = crate::proxy::hometier_protocol::proxy_port();
                                if let Some(csp) = upstream_headers
                                    .get("content-security-policy")
                                    .and_then(|v| v.to_str().ok())
                                {
                                    let sources = [format!("ws://127.0.0.1:{}", proxy_port)];
                                    csp_override = relax_csp(csp, &csp_hash, &sources);
                                }
                            }
                            builder = builder
                                .header("content-length", new_bytes.len().to_string());
                            full_body(Bytes::from(new_bytes))
                        }
                        Err(_) => {
                            crate::log_error!(format!(
                                "[DBG-PROXY][req#{}] rewrite_urls panic (catch_unwind), 透传原体 {}B",
                                Self::rid_of(ctx),
                                body_bytes.len()
                            ));
                            builder = builder
                                .header("content-length", body_bytes.len().to_string());
                            full_body(body_bytes)
                        }
                    }
                } else if status == StatusCode::NOT_MODIFIED || status == StatusCode::NO_CONTENT {
                    crate::log_debug!(format!(
                        "[DBG-PROXY][req#{}] {status} 透传空 body",
                        Self::rid_of(ctx)
                    ));
                    full_body(Bytes::new())
                } else if is_download {
                    // 附件：完整收集 → 保存到下载目录 → 同时原样返回给 iframe
                    let bytes = upstream.bytes().await.unwrap_or_default();
                    let dl_dir = crate::proxy::hometier_protocol::download_dir();
                    if let Some(dir) = dl_dir {
                        let raw_name = content_disposition
                            .as_deref()
                            .map(|cd| Self::extract_filename(cd, forward_url))
                            .unwrap_or_else(|| forward_url.to_string());
                        let safe_name = Self::sanitize_filename(&raw_name);
                        let saved_path = format!("{}/{}", dir, safe_name);
                        let path_buf = saved_path.clone();
                        let bytes_clone = bytes.clone();
                        tokio::spawn(async move {
                            let _ = tokio::fs::write(&path_buf, &bytes_clone).await;
                        });
                        {
                            let mut queue =
                                crate::proxy::hometier_protocol::pending_downloads()
                                    .lock()
                                    .unwrap();
                            queue.push(saved_path.clone());
                        }
                    }
                    builder = builder.header("content-length", bytes.len().to_string());
                    full_body(bytes)
                } else {
                    stream_body(upstream.bytes_stream())
                };

                let mut resp = builder.body(body).unwrap();
                if let Some(csp_override) = csp_override {
                    if csp_override.is_empty() {
                        resp.headers_mut().remove("content-security-policy");
                    } else if let Ok(hv) = csp_override.parse() {
                        resp.headers_mut()
                            .insert("content-security-policy", hv);
                    }
                }
                Ok(resp)
            }
            Err(e) => {
                crate::log_error!(format!(
                    "[DBG-PROXY][req#{}] 上游请求失败 {} {}",
                    Self::rid_of(ctx),
                    forward_url,
                    e
                ));
                Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .header("content-type", "text/plain; charset=utf-8")
                    .body(full_body(Bytes::from(format!(
                        "Proxy request failed: {}",
                        e
                    ))))
                    .unwrap())
            }
        }
    }
}
