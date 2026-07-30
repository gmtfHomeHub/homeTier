use crate::log::{self, LogLevel};

#[tauri::command]
pub async fn get_logs(level: Option<String>, since_seq: Option<u64>) -> Vec<log::LogEntry> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = crate::daemon::client::IpcClient::get_global();
        if client.ping().await {
            if let Ok(crate::daemon::ipc::IpcResponse::Ok { data }) =
                client.get_logs(level.as_deref(), since_seq).await
            {
                if let Some(json) = data {
                    if let Ok(logs) = serde_json::from_value::<Vec<log::LogEntry>>(json) {
                        return logs;
                    }
                }
            }
        }
    }

    // 回落：daemon IPC 不可达时返回 GUI 本地缓存日志
    let level_filter = level.as_deref().and_then(|l| match l.to_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    });
    log::get_all(level_filter)
}

#[tauri::command]
pub fn get_space_logs(space_id: String, level: Option<String>) -> Vec<log::LogEntry> {
    let level_filter = level.and_then(|l| match l.to_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    });
    log::get_by_space(&space_id, level_filter)
}

#[tauri::command]
pub async fn clear_logs() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = crate::daemon::client::IpcClient::get_global();
        if client.ping().await {
            let _ = client.clear_daemon_logs().await;
        }
    }
}