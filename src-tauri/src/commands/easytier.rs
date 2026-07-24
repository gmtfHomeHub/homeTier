use crate::daemon::{client::IpcClient, ipc::IpcResponse};

/// 获取 EasyTier 版本
#[tauri::command]
pub async fn get_easytier_version() -> Result<String, String> {
    let client = IpcClient::default_port();
    match client.get_version().await {
        Ok(IpcResponse::Ok { data }) => {
            data.and_then(|v| v.get("version").and_then(|s| s.as_str().map(|s| s.to_string())))
                .ok_or_else(|| "无法获取版本".into())
        }
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(format!("连接 daemon 失败: {}", e)),
    }
}

/// 检查 EasyTier 更新
#[tauri::command]
pub async fn check_easytier_update() -> Result<Vec<String>, String> {
    // TODO: 从远端获取可用版本列表
    Ok(vec![])
}

/// 升级 EasyTier
#[tauri::command]
pub async fn upgrade_easytier(version: String, source_path: Option<String>) -> Result<(), String> {
    let client = IpcClient::default_port();
    match client.upgrade(&version, source_path.as_deref()).await {
        Ok(IpcResponse::Ok { .. }) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(format!("连接 daemon 失败: {}", e)),
    }
}
