use std::sync::Arc;
use sha2::{Digest, Sha256};
use tauri::State;
use crate::proxy::{ActiveOrigin, ProxyKeyMap, ProxyServer};

#[tauri::command]
pub async fn get_proxy_url(
    proxy: State<'_, Arc<ProxyServer>>,
) -> Result<String, String> {
    Ok(proxy.proxy_url())
}

#[tauri::command]
pub async fn get_proxy_status(
    proxy: State<'_, Arc<ProxyServer>>,
) -> Result<serde_json::Value, String> {
    crate::log_debug!("获取代理状态");
    Ok(serde_json::json!({
        "running": true,
        "port": proxy.port,
        "proxy_url": proxy.proxy_url(),
    }))
}

#[tauri::command]
pub async fn register_proxy_key(
    url: String,
    key_map: State<'_, ProxyKeyMap>,
) -> Result<String, String> {
    crate::log_debug!(format!("注册代理 key: url={}", url));
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hasher.finalize();
    let key = hex::encode(&hash[..6]);

    key_map.write().await.insert(key.clone(), url);
    Ok(key)
}

#[tauri::command]
pub async fn set_proxy_source(
    url: String,
    origin: State<'_, ActiveOrigin>,
) -> Result<(), String> {
    crate::log_info!(format!("设置代理源: url={}", url));
    *origin.write().await = Some(url);
    Ok(())
}
