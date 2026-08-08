use crate::log::{self, LogCategory, LogFilter, LogLevel};

#[tauri::command]
pub async fn get_logs(level: Option<String>, since_seq: Option<u64>) -> Vec<log::LogEntry> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = crate::daemon::client::IpcClient::get_global();
        if client.ping().await {
            if let Ok(crate::daemon::ipc::IpcResponse::Ok { data }) =
                client.get_logs(level.as_deref(), since_seq, None).await
            {
                if let Some(json) = data {
                    if let Ok(logs) = serde_json::from_value::<Vec<log::LogEntry>>(json) {
                        return logs;
                    }
                }
            }
        }
    }

    // 回落：daemon IPC 不可达时，仅初始获取返回 GUI 本地日志
    if since_seq.is_none() {
        let level_filter = level.as_deref().and_then(parse_level);
        return log::get_all(level_filter);
    }

    vec![]
}

#[tauri::command]
pub async fn get_space_logs(space_id: String, level: Option<String>) -> Vec<log::LogEntry> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = crate::daemon::client::IpcClient::get_global();
        if client.ping().await {
            if let Ok(crate::daemon::ipc::IpcResponse::Ok { data }) =
                client.get_logs(level.as_deref(), None, Some(&space_id)).await
            {
                if let Some(json) = data {
                    if let Ok(logs) = serde_json::from_value::<Vec<log::LogEntry>>(json) {
                        return logs;
                    }
                }
            }
        }
    }

    let level_filter = level.as_deref().and_then(parse_level);
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

// ---- v2 复合查询 / 过滤清除 / 模块发现 ----

/// v2 复合查询，支持 level / space / module / category / keyword / since_seq / limit
#[tauri::command]
pub async fn query_logs(
    level: Option<String>,
    space_id: Option<String>,
    module: Option<String>,
    category: Option<String>,
    keyword: Option<String>,
    since_seq: Option<u64>,
    limit: Option<usize>,
) -> Vec<log::LogEntry> {
    let filter = LogFilter {
        level: level.as_deref().and_then(parse_level),
        space_id,
        module,
        category: category.as_deref().and_then(parse_category),
        keyword,
        since_seq,
        limit,
    };
    log::query(&filter)
}

/// 返回当前缓存中的活跃模块列表，供前端 UI 渲染模块筛选器
#[tauri::command]
pub async fn get_log_modules() -> Vec<String> {
    log::active_modules()
}

/// v2 清除：filter 为空则清空全部；否则仅清除匹配项
#[tauri::command]
pub async fn clear_logs_filtered(
    level: Option<String>,
    space_id: Option<String>,
    module: Option<String>,
    category: Option<String>,
    keyword: Option<String>,
) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = crate::daemon::client::IpcClient::get_global();
        if client.ping().await {
            let _ = client.clear_daemon_logs().await;
        }
    }
    let filter = LogFilter {
        level: level.as_deref().and_then(parse_level),
        space_id,
        module,
        category: category.as_deref().and_then(parse_category),
        keyword,
        since_seq: None,
        limit: None,
    };
    log::clear_filtered(&filter);
}

// ---- 工具 ----

fn parse_level(s: &str) -> Option<LogLevel> {
    match s.to_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}

fn parse_category(s: &str) -> Option<LogCategory> {
    match s.to_lowercase().as_str() {
        "system" => Some(LogCategory::System),
        "network" => Some(LogCategory::Network),
        "webrtc" => Some(LogCategory::WebRTC),
        "data" => Some(LogCategory::Data),
        "proxy" => Some(LogCategory::Proxy),
        "daemon" => Some(LogCategory::Daemon),
        "space" => Some(LogCategory::Space),
        "server" => Some(LogCategory::Server),
        _ => None,
    }
}