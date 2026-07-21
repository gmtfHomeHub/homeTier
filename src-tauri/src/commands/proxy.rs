use std::sync::Arc;
use tauri::State;
use crate::proxy::ProxyServer;

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
    Ok(serde_json::json!({
        "running": true,
        "port": proxy.port,
        "proxy_url": proxy.proxy_url(),
    }))
}
