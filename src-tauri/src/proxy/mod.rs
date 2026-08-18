pub mod hometier_protocol;
pub mod plugin;
pub mod plugins;
pub mod rewriter;
pub mod server;
pub mod ws_proxy;

pub use plugin::{PluginChain, ProxyHandler, ProxyPlugin, RequestContext};
pub use server::ProxyServer;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::RwLock;

/// 代理 key → 源地址 映射
pub type ProxyKeyMap = Arc<RwLock<HashMap<String, String>>>;

/// 兜底缓存的当前活跃源地址（覆盖 fetch('/api') 等绝对路径动态请求）
pub type ActiveOrigin = Arc<RwLock<Option<String>>>;

/// 自签/私有 CA 证书仓库：{app_data_dir}/ca_certs/*.pem（存储原始 PEM 字节），
/// 供 http/https 与 wss 上游连接信任（内网自签证书应用）
static CA_STORE: OnceLock<Mutex<Vec<Vec<u8>>>> = OnceLock::new();

/// 返回可信任的 reqwest 证书（每次从 PEM 重建，次数少、开销可忽略）
pub fn proxy_ca_certs() -> Vec<reqwest::Certificate> {
    let store = CA_STORE
        .get()
        .map(|s| s.lock().unwrap().clone())
        .unwrap_or_default();
    store
        .iter()
        .filter_map(|pem| reqwest::Certificate::from_pem(pem).ok())
        .collect()
}

/// 返回可信任的 rustls 证书（供 wss 上游 TLS 握手使用）
pub fn proxy_ca_der() -> Vec<rustls::pki_types::CertificateDer<'static>> {
    use std::io::Cursor;
    let store = CA_STORE
        .get()
        .map(|s| s.lock().unwrap().clone())
        .unwrap_or_default();
    store
        .iter()
        .flat_map(|pem| {
            let mut cursor = Cursor::new(pem);
            let items: Vec<_> = rustls_pemfile::certs(&mut cursor).collect();
            items
        })
        .filter_map(|r| r.ok())
        .collect()
}

/// 加载 {dir}/*.pem 作为代理上游信任根证书（幂等，可多次调用累加）
pub fn load_proxy_ca_certs(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut loaded: Vec<Vec<u8>> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pem") {
            continue;
        }
        match std::fs::read(&path) {
            Ok(bytes) => match reqwest::Certificate::from_pem(&bytes) {
                Ok(_) => loaded.push(bytes),
                Err(e) => crate::log_error!(format!(
                    "加载 CA 证书失败 {}: {}",
                    path.display(),
                    e
                )),
            },
            Err(e) => crate::log_error!(format!(
                    "读取 CA 证书失败 {}: {}",
                    path.display(),
                    e
                )),
        }
    }
    if loaded.is_empty() {
        return;
    }
    let store = CA_STORE.get_or_init(|| Mutex::new(Vec::new()));
    let mut store = store.lock().unwrap();
    store.extend(loaded);
    crate::log_info!(format!("已加载 {} 个自签 CA 证书", store.len()));
}
