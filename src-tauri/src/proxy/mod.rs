use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use http_body_util::BodyExt;
use encoding_rs::Encoding;
use regex::Regex;
use reqwest::Client;
use std::borrow::Cow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};

/// 代理服务
pub struct ProxyServer {
    pub port: u16,
    /// 持有 tokio 运行时，确保 accept 循环持续运行
    _runtime: tokio::runtime::Runtime,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ProxyServer {
    /// 启动代理服务器，监听 127.0.0.1 上的随机空闲端口
    /// 同步函数，内部创建独立 tokio 运行时
    pub fn start() -> Result<Self, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| format!("创建 tokio 运行时失败: {}", e))?;

        let (shutdown_tx, port) = rt.block_on(async {
            let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
            let listener = TcpListener::bind(addr)
                .await
                .map_err(|e| format!("代理绑定端口失败: {}", e))?;
            let port = listener.local_addr()
                .map_err(|e| format!("获取端口失败: {}", e))?
                .port();

            let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
            let shutdown_flag = Arc::new(Mutex::new(false));
            let shutdown_flag_clone = shutdown_flag.clone();

            let client = Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

            let proxy_prefix = format!("http://127.0.0.1:{}", port);

            tokio::spawn(async move {
                let shutdown_fut = async { shutdown_rx.await.ok() };
                tokio::pin!(shutdown_fut);
                let proxy_prefix = proxy_prefix.clone();

                loop {
                    tokio::select! {
                        accept_result = listener.accept() => {
                            match accept_result {
                                Ok((stream, _)) => {
                                    let io = TokioIo::new(stream);
                                    let client = client.clone();
                                    let proxy_prefix = proxy_prefix.clone();
                                    let shutdown_flag = shutdown_flag_clone.clone();
                                    tokio::spawn(async move {
                                        let service = service_fn(move |req| {
                                            handle_proxy(req, client.clone(), proxy_prefix.clone())
                                        });
                                        let conn = http1::Builder::new()
                                            .serve_connection(io, service);
                                        tokio::select! {
                                            _ = conn => {}
                                            _ = async {
                                                loop {
                                                    if *shutdown_flag.lock().await {
                                                        break;
                                                    }
                                                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                                }
                                            } => {}
                                        }
                                    });
                                }
                                Err(e) => {
                                    eprintln!("代理接受连接错误: {}", e);
                                }
                            }
                        }
                        _ = &mut shutdown_fut => {
                            *shutdown_flag_clone.lock().await = true;
                            break;
                        }
                    }
                }
            });

            Ok::<_, String>((shutdown_tx, port))
        })?;

        Ok(Self {
            port,
            _runtime: rt,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    /// 获取代理 URL 前缀
    pub fn proxy_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// 构建代理请求 URL
    pub fn proxy_url_for(&self, target_url: &str) -> String {
        format!(
            "http://127.0.0.1:{}/proxy?url={}",
            self.port,
            urlencoding::encode(target_url)
        )
    }

    /// 关闭代理服务器
    pub fn shutdown(&mut self) {
        crate::log_info!(format!("代理服务器关闭: port={}", self.port));
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for ProxyServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 从响应头中检测字符编码
/// 从 Content-Type 头的 charset 参数提取编码名，用 encoding_rs 解析
/// 若未指定或无法识别，默认返回 UTF-8
fn detect_charset(headers: &hyper::HeaderMap) -> &'static Encoding {
    let charset_str = headers.get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|ct| {
            // 提取 charset=XXX
            let re = Regex::new(r"charset\s*=\s*([^\s;]+)").ok()?;
            re.captures(ct)?.get(1).map(|m| m.as_str())
        })
        .unwrap_or("utf-8");

    // 用 encoding_rs 查找编码，默认 UTF-8
    Encoding::for_label(charset_str.as_bytes()).unwrap_or(encoding_rs::UTF_8)
}

/// 处理代理请求
async fn handle_proxy(
    req: Request<Incoming>,
    client: Client,
    proxy_prefix: String,
) -> Result<Response<http_body_util::Full<hyper::body::Bytes>>, hyper::Error> {
    let uri = req.uri().clone();
    let query = uri.query().unwrap_or("");

    eprintln!("[proxy] handle_proxy called: {:?}", uri);

    // 解析目标 URL
    let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes()).collect();
    let target_url = match params.get("url") {
        Some(url) => {
            eprintln!("[proxy] 代理请求: {}", url);
            crate::log_info!(format!("代理请求: {}", url));
            url.to_string()
        }
        None => {
            eprintln!("[proxy] 代理请求缺少 url 参数");
            crate::log_warn!("代理请求缺少 url 参数");
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(http_body_util::Full::new(hyper::body::Bytes::from(
                    "Missing 'url' parameter",
                )))
                .unwrap());
        }
    };

    // 构建转发请求
    let forward_url = target_url.clone();
    let mut req_builder = client.get(&forward_url);

    // 转发原始请求头（排除 hop-by-hop 头）
    let hop_by_hop = [
        "host", "connection", "keep-alive", "proxy-authenticate",
        "proxy-authorization", "te", "trailers", "transfer-encoding", "upgrade",
    ];
    for (key, value) in req.headers() {
        let key_lower = key.as_str().to_lowercase();
        if !hop_by_hop.contains(&key_lower.as_str()) {
            req_builder = req_builder.header(key, value);
        }
    }

    // 发送转发请求
    match req_builder.send().await {
        Ok(resp) => {
            let status = resp.status();
            let content_length = resp.content_length().unwrap_or(0);
            crate::log_info!(format!("代理响应: {} -> {} ({} bytes)", status, target_url, content_length));

            let headers = resp.headers().clone();

            // 构建响应
            let mut builder = Response::builder().status(status);

            for (key, value) in &headers {
                let key_lower = key.as_str().to_lowercase();

                // 跳过阻止 iframe 加载的响应头
                if key_lower == "x-frame-options" {
                    continue;
                }

                // 从 Content-Security-Policy 中移除 frame-ancestors
                if key_lower == "content-security-policy" {
                    if let Ok(val) = value.to_str() {
                        let filtered: Vec<&str> = val
                            .split(';')
                            .map(|s| s.trim())
                            .filter(|s| !s.starts_with("frame-ancestors"))
                            .collect();
                        let joined = filtered.join("; ");
                        if !joined.is_empty() {
                            builder = builder.header(key, joined);
                        }
                    }
                    continue;
                }

                // 跳过 Content-Length, Transfer-Encoding 和 Content-Encoding
                // URL 重写后 Content-Length 会变化，需在重写后重新设置
                // Transfer-Encoding 由 hyper 自动管理
                // Content-Encoding: reqwest 已自动解压，响应体是明文，无需再告知浏览器解压
                if key_lower == "content-length" || key_lower == "transfer-encoding" || key_lower == "content-encoding" {
                    continue;
                }

                // 处理 Content-Type: 将 charset 强制替换为 utf-8
                // String::from_utf8_lossy 已将响应体转换为 UTF-8，需告知浏览器
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

            // 设置 CORS 头，允许 iframe 跨域
            builder = builder
                .header("access-control-allow-origin", "*")
                .header("access-control-allow-methods", "GET, POST, OPTIONS")
                .header("access-control-allow-headers", "*");

            // 读取响应体
            let body_bytes = resp.bytes().await.unwrap_or_default();

            // 判断是否需要重写 URL（仅对 HTML/CSS 内容）
            let content_type = headers.get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_lowercase();

            let body = if content_type.contains("text/html") || content_type.contains("text/css") {
                eprintln!("[proxy] 开始 URL 重写: content_type={}, size={} bytes", content_type, body_bytes.len());
                let encoding = detect_charset(&headers);
                let (body_str, _, _) = encoding.decode(&body_bytes);
                let rewritten = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    rewrite_html_urls(&body_str, &target_url, &proxy_prefix)
                }));

                match rewritten {
                    Ok(cow) => {
                        let new_bytes = cow.as_bytes().to_vec();
                        eprintln!("[proxy] URL 重写完成: {} -> {} bytes", target_url, new_bytes.len());
                        builder = builder.header("content-length", new_bytes.len().to_string());
                        http_body_util::Full::new(hyper::body::Bytes::from(new_bytes))
                    }
                    Err(e) => {
                        eprintln!("[proxy] URL 重写 panic: {:?}", e);
                        crate::log_error!(format!("URL 重写异常: {:?}", e));
                        // 回退到原始内容
                        http_body_util::Full::new(hyper::body::Bytes::from(body_bytes.to_vec()))
                    }
                }
            } else {
                http_body_util::Full::new(hyper::body::Bytes::from(body_bytes))
            };

            Ok(builder.body(body).unwrap())
        }
        Err(e) => {
            // 转发失败
            crate::log_error!(format!("代理请求失败: {} -> {}", e, target_url));
            let err_msg = format!("代理请求失败: {}", e);
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("content-type", "text/plain; charset=utf-8")
                .body(http_body_util::Full::new(hyper::body::Bytes::from(err_msg)))
                .unwrap())
        }
    }
}

/// 将 HTML/CSS 内容中的子资源 URL 重写为代理地址
fn rewrite_html_urls<'a>(content: &'a str, base_url: &'a str, proxy_prefix: &'a str) -> Cow<'a, str> {
    // 提取 base URL 的协议和域名部分，用于解析相对路径
    let base_origin = base_url
        .trim_end_matches('/')
        .to_string();

    // 构建代理 URL 编码函数
    let encode_proxy = |url: &str| -> String {
        format!("{}/proxy?url={}", proxy_prefix, urlencoding::encode(url))
    };

    // 判断是否为绝对 URL（http/https）
    let is_absolute = |url: &str| -> bool {
        url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
    };

    // 解析相对路径为绝对路径
    let resolve_url = |url: &str| -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else if url.starts_with("//") {
            // 协议相对 URL
            if base_url.starts_with("https") {
                format!("https:{}", url)
            } else {
                format!("http:{}", url)
            }
        } else if url.starts_with('/') {
            // 绝对路径
            let origin = base_origin.trim_end_matches('/');
            format!("{}{}", origin, url)
        } else {
            // 相对路径 - 基于 base_url 的目录
            let base_dir = if base_url.ends_with('/') {
                base_url.to_string()
            } else {
                let last_slash = base_url.rfind('/');
                match last_slash {
                    Some(pos) => base_url[..=pos].to_string(),
                    None => format!("{}/", base_url),
                }
            };
            format!("{}{}", base_dir, url)
        }
    };

    // 1. 重写 HTML 标签属性中的 URL
    let re_attr = Regex::new(
        r#"(?i)(\b(?:src|href|action|poster|data-src|data-href|data-url)\s*=\s*)"([^"]+)"|(\b(?:src|href|action|poster|data-src|data-href|data-url)\s*=\s*)'([^']+)'"#
    ).unwrap();
    let mut result = content.to_string();

    // 处理 HTML 属性中的 URL
    result = re_attr.replace_all(&result, |caps: &regex::Captures| {
        // 双引号版本: $1="$2" 或单引号版本: $3'$4'
        let (prefix, url) = if caps.get(2).is_some() {
            (caps.get(1).unwrap().as_str().to_string(), caps.get(2).unwrap().as_str().to_string())
        } else {
            (caps.get(3).unwrap().as_str().to_string(), caps.get(4).unwrap().as_str().to_string())
        };

        // 跳过已代理的 URL
        if url.starts_with(proxy_prefix) || url.contains("/proxy?url=") {
            return format!("{}{}", prefix, url);
        }

        // 只重写 http/https 协议或相对路径
        if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") || url.starts_with('/') || !url.contains("://") {
            let absolute = resolve_url(&url);
            if is_absolute(&absolute) {
                return format!("{}{}", prefix, encode_proxy(&absolute));
            }
        }
        format!("{}{}", prefix, url)
    }).into_owned();

    // 2. 重写 srcset 属性（多个 URL + 描述符）
    let re_srcset = Regex::new(
        r#"(?i)(\bsrcset\s*=\s*)"([^"]+)"|(\bsrcset\s*=\s*)'([^']+)'"#
    ).unwrap();
    result = re_srcset.replace_all(&result, |caps: &regex::Captures| {
        let (prefix, value) = if caps.get(2).is_some() {
            (caps.get(1).unwrap().as_str().to_string(), caps.get(2).unwrap().as_str().to_string())
        } else {
            (caps.get(3).unwrap().as_str().to_string(), caps.get(4).unwrap().as_str().to_string())
        };

        // 重写 srcset 中的每个 URL
        let rewritten = value.split(',')
            .map(|part| {
                let part = part.trim();
                // 分割 URL 和描述符（如 "photo.jpg 1x"）
                let parts: Vec<&str> = part.splitn(2, |c: char| c.is_whitespace()).collect();
                let url = parts[0].trim();
                let descriptor = if parts.len() > 1 { parts[1] } else { "" };

                if url.starts_with(proxy_prefix) || url.contains("/proxy?url=") {
                    return part.to_string();
                }
                if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") || url.starts_with('/') || !url.contains("://") {
                    let absolute = resolve_url(&url);
                    if is_absolute(&absolute) {
                        if descriptor.is_empty() {
                            return encode_proxy(&absolute);
                        }
                        return format!("{} {}", encode_proxy(&absolute), descriptor);
                    }
                }
                part.to_string()
            })
            .collect::<Vec<_>>()
            .join(", ");

        format!("{}{}", prefix, rewritten)
    }).into_owned();

    // 3. 重写 CSS url() 引用（包括 <style> 标签内和 style 属性）
    let re_css_url = Regex::new(r#"(?i)url\(\s*['"]?([^'")\s]+)['"]?\s*\)"#).unwrap();
    result = re_css_url.replace_all(&result, |caps: &regex::Captures| {
        let url = caps.get(1).unwrap().as_str().trim();

        if url.starts_with(proxy_prefix) || url.contains("/proxy?url=") {
            return format!("url({})", url);
        }
        if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") || url.starts_with('/') || !url.contains("://") {
            let absolute = resolve_url(&url);
            if is_absolute(&absolute) {
                return format!("url({})", encode_proxy(&absolute));
            }
        }
        format!("url({})", url)
    }).into_owned();

    // 4. 重写 CSS @import 语句
    let re_import = Regex::new(r#"(?i)(@import\s+)"([^"]+)"|(@import\s+)'([^']+)'"#).unwrap();
    result = re_import.replace_all(&result, |caps: &regex::Captures| {
        let (prefix, url) = if caps.get(2).is_some() {
            (caps.get(1).unwrap().as_str().to_string(), caps.get(2).unwrap().as_str().to_string())
        } else {
            (caps.get(3).unwrap().as_str().to_string(), caps.get(4).unwrap().as_str().to_string())
        };

        if url.starts_with(proxy_prefix) || url.contains("/proxy?url=") {
            return format!("{}{}", prefix, url);
        }
        if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") || url.starts_with('/') || !url.contains("://") {
            let absolute = resolve_url(&url);
            if is_absolute(&absolute) {
                return format!("{}{}", prefix, encode_proxy(&absolute));
            }
        }
        format!("{}{}", prefix, url)
    }).into_owned();

    // 5. 替换 <meta charset> 标签中的编码为 UTF-8
    // 匹配 <meta charset="XXX"> 或 <meta charset='XXX'>
    let re_meta_charset = Regex::new(
        r#"(?i)(<meta\s+[^>]*?charset\s*=\s*['"])([^'">\s]+)(['"])"#
    ).unwrap();
    result = re_meta_charset.replace_all(&result, "${1}UTF-8${3}").into_owned();

    // 6. 替换 <meta http-equiv="Content-Type" content="...charset=XXX..."> 中的编码
    let re_meta_hc = Regex::new(
        r#"(?i)(<meta\s+[^>]*?http-equiv\s*=\s*['"]?content-type['"]?\s+[^>]*?charset\s*=\s*)([^\s"';>]+)"#
    ).unwrap();
    result = re_meta_hc.replace_all(&result, "${1}UTF-8").into_owned();

    Cow::Owned(result)
}
