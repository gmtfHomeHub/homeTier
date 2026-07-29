use crate::log::{self, LogLevel};

#[tauri::command]
pub async fn get_logs(level: Option<String>) -> Vec<log::LogEntry> {
    let level_filter = level.as_deref().and_then(|l| match l.to_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    });

    let mut logs = log::get_all(level_filter);

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = crate::daemon::client::IpcClient::get_global();
        if client.ping().await {
            if let Ok(crate::daemon::ipc::IpcResponse::Ok { data }) =
                client.get_daemon_logs(level.as_deref()).await
            {
                if let Some(json) = data {
                    if let Ok(mut daemon_logs) =
                        serde_json::from_value::<Vec<log::LogEntry>>(json)
                    {
                        logs.append(&mut daemon_logs);
                        logs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                    }
                }
            }
        }
    }

    logs
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
    log::clear();

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = crate::daemon::client::IpcClient::get_global();
        client.clear_daemon_logs().await.ok();
    }
}