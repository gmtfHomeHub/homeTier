use tauri::State;
use crate::daemon::{client::IpcClient, ipc::IpcResponse};
use crate::app::daemon::DaemonReadyState;

#[tauri::command]
pub fn is_daemon_ready(ready_state: State<'_, DaemonReadyState>) -> bool {
    ready_state.ready.load(std::sync::atomic::Ordering::SeqCst)
}

#[tauri::command]
pub fn get_daemon_error_reason(ready_state: State<'_, DaemonReadyState>) -> Option<String> {
    ready_state.reason.lock().ok()?.clone()
}

#[tauri::command]
pub async fn get_daemon_logs(level: Option<String>) -> Result<Vec<crate::log::LogEntry>, String> {
    crate::log_debug!("获取守护进程日志");
    let client = IpcClient::get_global();
    match client.get_logs(level.as_deref(), None, None).await {
        Ok(IpcResponse::Ok { data }) => {
            match data {
                Some(v) => serde_json::from_value(v).map_err(|e| format!("反序列化日志失败: {}", e)),
                None => Ok(vec![]),
            }
        }
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn check_easytier_binary() -> Result<serde_json::Value, String> {
    crate::log_info!("检查 EasyTier 二进制");
    let client = IpcClient::get_global();
    match client.check_binary().await {
        Ok(IpcResponse::Ok { data }) => Ok(data.unwrap_or(serde_json::Value::Null)),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
    }
}