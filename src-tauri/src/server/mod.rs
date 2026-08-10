use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    compression::CompressionLayer,
};
use tokio::net::TcpListener;
use tracing::Span;

use crate::config::AppConfig;
use crate::db::Database;
use crate::easytier::EasyTierManager;
use crate::proxy::server::ProxyServer;
use crate::server::event::GlobalEventBus;
use crate::space::manager::SpaceManager;

pub mod auth;
pub mod event;
pub mod routes;
pub mod system_apps;
pub mod ws;

pub mod assets;

pub const SERVER_CONF_TEMPLATE: &str = r#"# homeTier 服务器模式配置
# 初次启动自动生成；修改保存后热更新生效（自动检测 mtime）。

# 绑定地址与端口
SERVER_BIND=0.0.0.0
SERVER_PORT=9339

# 前端静态资源目录（相对于运行目录或绝对路径；目录不存在时自动回退到编译时嵌入的 dist）
SERVER_STATIC_DIR=./dist

# TLS（cert/key 均为 PEM 格式文件路径，留空则纯 HTTP）
SERVER_TLS=false
SERVER_TLS_CERT=
SERVER_TLS_KEY=

# Cookie 签名密钥（空则每次启动自动生成 32 字节随机——重启后旧 cookie 全部失效）
SERVER_AUTH_SECRET=

# CORS 允许来源（逗号分隔），* 允许所有
SERVER_CORS_ORIGIN=*

# 第三方 Web 代理路径前缀
SERVER_PROXY_PREFIX=/proxy
"#;

pub struct AppState {
    pub db: Arc<Database>,
    pub space_manager: Arc<SpaceManager>,
    pub easy_tier: Arc<EasyTierManager>,
    pub config: Arc<AppConfig>,
    pub auth_secret: String,
    pub event_bus: Arc<GlobalEventBus>,
    pub proxy_server: Arc<ProxyServer>,
    pub file_manager: Arc<crate::file::FileTransferManager>,
    pub file_registry: Arc<crate::file::FileServerRegistry>,
    pub config_store: Arc<crate::config_store::ConfigStoreService>,
    pub data_dir: std::path::PathBuf,
}

pub fn init_server_config(data_dir: &std::path::Path) -> Arc<AppConfig> {
    let conf_path = data_dir.join("server.conf");
    let config = AppConfig::new(conf_path, Some(SERVER_CONF_TEMPLATE.to_string()), None);
    config.load();
    Arc::new(config)
}

/// 启动内部 HTTP 代理（与桌面模式一致：CORS/iframe 绕过 + HTTP 反向代理 + WebSocket 隧道）
pub fn init_proxy_server() -> Arc<ProxyServer> {
    use std::collections::HashMap;
    use tokio::sync::RwLock;
    use crate::proxy::plugin::{HttpReverseProxyHandler, ProxyHandler};
    use crate::proxy::plugins::{CorsPlugin, HttpForwardPlugin, HttpsTunnelPlugin, IframeBypassPlugin};

    let active_origin: crate::proxy::ActiveOrigin = Arc::new(RwLock::new(None));
    let key_map: crate::proxy::ProxyKeyMap = Arc::new(RwLock::new(HashMap::new()));
    let http_forward = HttpForwardPlugin::new(key_map.clone(), active_origin.clone())
        .map_err(|e| format!("创建 HttpForwardPlugin 失败: {}", e))
        .expect("HttpForwardPlugin 初始化失败");
    let handlers: Vec<Arc<dyn ProxyHandler>> = vec![
        Arc::new(HttpsTunnelPlugin),
        Arc::new(http_forward),
        Arc::new(HttpReverseProxyHandler::new()
            .map_err(|e| format!("创建 HttpReverseProxyHandler 失败: {}", e))
            .expect("HttpReverseProxyHandler 初始化失败")),
    ];
    let proxy_server = ProxyServer::start(
        vec![
            Arc::new(CorsPlugin::new()),
            Arc::new(IframeBypassPlugin),
        ],
        handlers,
    ).map_err(|e| format!("启动代理服务器失败: {}", e))
        .expect("代理服务器启动失败");
    crate::log_info!(format!("服务器模式代理已启动: port={}", proxy_server.port()));
    Arc::new(proxy_server)
}

fn cors_layer(config: &AppConfig) -> CorsLayer {
    let origin = config.get_str("SERVER_CORS_ORIGIN", "*");
    if origin == "*" {
        // tower-http: allow_origin(Any) 与 allow_credentials(true) 组合会 panic，
        // 通配时只允许任意来源（浏览器同源请求不受影响，跨域场景需显式配置来源列表）
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        let origins: Vec<_> = origin
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
            .allow_credentials(true)
    }
}

async fn trace_layer(
    State(_app_state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let trace_id = extract_trace_id(req.headers())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    
    let mut req = req;
    req.extensions_mut().insert(trace_id.clone());

    let response = next.run(req).await;
    
    let mut response = response;
    response.headers_mut().insert(
        "x-trace-id",
        HeaderValue::from_str(&trace_id).unwrap_or(HeaderValue::from_static("")),
    );
    
    Ok(response)
}

fn extract_trace_id(headers: &HeaderMap) -> Option<String> {
    const TRACE_HEADER: &str = "x-trace-id";
    headers
        .get(TRACE_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

async fn auth_layer(
    state: Arc<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let path = req.uri().path().to_string();
    if auth::is_static_resource(&path) {
        return Ok(next.run(req).await);
    }

    let fingerprint = auth::extract_fingerprint(req.headers());

    match auth::verify_request(req.headers(), &state.auth_secret) {
        true => Ok(next.run(req).await),
        false => {
            let new_token = auth::generate_cookie_value(&fingerprint, &state.auth_secret);
            let mut response = Response::new(axum::body::Body::empty());
            *response.status_mut() = axum::http::StatusCode::UNAUTHORIZED;
            let secure = state.config.get_bool("SERVER_TLS", false);
            auth::set_cookie_header(&mut response, &new_token, secure);
            Ok(response)
        }
    }
}

pub async fn start_server(
    bind: &str,
    port: u16,
    static_dir: String,
    app_state: Arc<AppState>,
) -> Result<(), String> {
    let state_for_auth = Arc::clone(&app_state);
    let cors_layer = cors_layer(&app_state.config);

    let router = Router::new()
        .nest("/api/cmd", routes::cmd_router(Arc::clone(&app_state)))
        .fallback_service(routes::static_file_handler(static_dir))
        .layer(CompressionLayer::new())
        .layer(cors_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http()
            .make_span_with(|req: &Request| {
                let trace_id = req.extensions().get::<String>().cloned()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                tracing::info_span!("http_request", method = %req.method(), uri = %req.uri(), trace_id = %trace_id)
            })
            .on_request(|_req: &Request, _span: &Span| {
                tracing::debug!("request started");
            })
            .on_response(|_resp: &Response, _latency: std::time::Duration, _span: &Span| {
                tracing::debug!("request completed");
            }))
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&app_state),
            trace_layer,
        ))
        .layer(axum::middleware::from_fn(move |req, next| {
            let state = Arc::clone(&state_for_auth);
            async move { auth_layer(state, req, next).await }
        }));

    let addr = format!("{}:{}", bind, port);
    let listener = TcpListener::bind(&addr).await.map_err(|e| format!("端口绑定失败: {}", e))?;

    let tls_enabled = app_state.config.get_bool("SERVER_TLS", false);
    if tls_enabled {
        let cert_path = app_state.config.get_str("SERVER_TLS_CERT", "");
        let key_path = app_state.config.get_str("SERVER_TLS_KEY", "");
        if cert_path.is_empty() || key_path.is_empty() {
            return Err("SERVER_TLS=true 但 SERVER_TLS_CERT/SERVER_TLS_KEY 未配置".to_string());
        }
        let tls_acceptor =
            tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(build_tls_acceptor(&cert_path, &key_path)?));
        tracing::info!("服务器启动: https://{}", addr);
        let tls_listener = TlsListener {
            listener,
            acceptor: tls_acceptor,
        };
        axum::serve(tls_listener, router)
            .await
            .map_err(|e| format!("TLS 服务器启动失败: {}", e))
    } else {
        tracing::info!("服务器启动: http://{}", addr);
        axum::serve(listener, router)
            .await
            .map_err(|e| format!("服务器启动失败: {}", e))
    }
}

/// TLS 包装监听器：接受 TCP 连接后执行 TLS 握手，交给 axum::serve。
struct TlsListener {
    listener: tokio::net::TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
}

impl axum::serve::Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
    type Addr = std::net::SocketAddr;

    fn accept(
        &mut self,
    ) -> impl std::future::Future<Output = (Self::Io, Self::Addr)> + Send {
        async {
            loop {
                let (stream, addr) = match self.listener.accept().await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("TLS accept error: {}", e);
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }
                };
                match self.acceptor.accept(stream).await {
                    Ok(tls_stream) => return (tls_stream, addr),
                    Err(e) => {
                        tracing::warn!("TLS handshake failed from {}: {}", addr, e);
                    }
                }
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}

fn build_tls_acceptor(
    cert_path: &str,
    key_path: &str,
) -> Result<rustls::server::ServerConfig, String> {
    use std::fs::File;
    use std::io::BufReader;

    let cert_file = File::open(cert_path).map_err(|e| format!("打开证书失败 {}: {}", cert_path, e))?;
    let certs: Vec<rustls_pki_types::CertificateDer<'static>> =
        rustls_pemfile::certs(&mut BufReader::new(cert_file))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("解析证书失败: {}", e))?;
    if certs.is_empty() {
        return Err("证书文件为空".to_string());
    }

    let key_file = File::open(key_path).map_err(|e| format!("打开私钥失败 {}: {}", key_path, e))?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(key_file))
        .map_err(|e| format!("解析私钥失败: {}", e))?
        .ok_or_else(|| "私钥文件为空".to_string())?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("TLS 配置失败: {}", e))?;
    Ok(config)
}