use std::sync::Arc;
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
    let hash = crate::crypto::sha256(url.as_bytes());
    let key = hex::encode(&hash[..6]);

    // 记录上游协议（http/https），供 hometierproxy 转发与 WSS 注入使用
    if let Some((scheme_end, rest)) = url.find("://").map(|p| (p + 3, &url[p + 3..])) {
        let host_key = rest
            .split('/')
            .next()
            .unwrap_or(rest)
            .to_string();
        if !host_key.is_empty() {
            let scheme = if url[..scheme_end - 3].eq_ignore_ascii_case("https") {
                "https"
            } else {
                "http"
            };
            crate::proxy::hometier_protocol::upstream_schemes()
                .lock()
                .unwrap()
                .insert(host_key, scheme.to_string());
        }
    }

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

#[tauri::command]
pub async fn set_device_mode(mode: String) -> Result<(), String> {
    crate::log_info!(format!("设置设备仿真模式: {}", mode));
    crate::proxy::hometier_protocol::set_device_mode(&mode);
    Ok(())
}

#[tauri::command]
pub async fn get_pending_downloads() -> Result<Vec<String>, String> {
    let mut queue = crate::proxy::hometier_protocol::pending_downloads()
        .lock()
        .unwrap();
    Ok(std::mem::take(&mut *queue))
}
