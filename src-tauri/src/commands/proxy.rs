use tauri::State;
use crate::proxy::ProxyServer;
use std::sync::Arc;

#[tauri::command]
pub async fn get_proxy_url(
    proxy: tauri::State<'_, std::sync::Arc<crate::proxy::ProxyServer>>,
) -> Result<String, String> {
    let url = proxy.proxy_url();
    eprintln!("[get_proxy_url] -> {}", url);
    Ok(url)
}