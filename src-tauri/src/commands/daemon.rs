use std::sync::Arc;
use tauri::State;
use crate::daemon::{client::IpcClient, service::get_service_manager, ipc::IpcResponse};
use crate::DaemonReadyState;

/// 查询 daemon 是否就绪（前端轮询用，不依赖事件系统）
#[tauri::command]
pub fn is_daemon_ready(ready_state: State<'_, DaemonReadyState>) -> bool {
    ready_state.ready.load(std::sync::atomic::Ordering::SeqCst)
}

/// 获取 daemon 启动失败的原因（供前端展示具体错误信息）
#[tauri::command]
pub fn get_daemon_error_reason(ready_state: State<'_, DaemonReadyState>) -> Option<String> {
    ready_state.reason.lock().ok()?.clone()
}

/// 检查守护进程是否正在运行
#[tauri::command]
pub async fn check_daemon_running() -> Result<bool, String> {
    crate::log_debug!("检查守护进程运行状态");
    let client = IpcClient::get_global();
    Ok(client.ping().await)
}

/// 获取守护进程状态
#[tauri::command]
pub async fn get_daemon_status() -> Result<serde_json::Value, String> {
    crate::log_debug!("获取守护进程状态");
    let client = IpcClient::get_global();
    match client.get_status().await {
        Ok(IpcResponse::Ok { data }) => Ok(data.unwrap_or(serde_json::Value::Null)),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
    }
}

/// 连接到空间（通过守护进程）
#[tauri::command]
pub async fn daemon_connect_space(space_id: String) -> Result<(), String> {
    crate::log_info!(format!("守护进程连接空间: {}", space_id));
    let client = IpcClient::get_global();
    match client.connect_space(&space_id, serde_json::json!({})).await {
        Ok(IpcResponse::Ok { .. }) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
    }
}

/// 断开空间连接（通过守护进程）
#[tauri::command]
pub async fn daemon_disconnect_space(space_id: String) -> Result<(), String> {
    crate::log_info!(format!("守护进程断开空间: {}", space_id));
    let client = IpcClient::get_global();
    match client.disconnect_space(&space_id).await {
        Ok(IpcResponse::Ok { .. }) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
    }
}

/// 获取已连接的空间列表（通过守护进程）
#[tauri::command]
pub async fn daemon_list_spaces() -> Result<Vec<String>, String> {
    crate::log_debug!("守护进程获取空间列表");
    let client = IpcClient::get_global();
    match client.list_spaces().await {
        Ok(IpcResponse::Ok { data }) => {
            Ok(data.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default())
        }
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
    }
}

/// 安装守护进程服务
#[tauri::command]
pub async fn install_daemon_service() -> Result<(), String> {
    crate::log_info!("安装守护进程服务");
    let manager = get_service_manager();
    manager.install()
}

/// 卸载守护进程服务
#[tauri::command]
pub async fn uninstall_daemon_service() -> Result<(), String> {
    crate::log_info!("卸载守护进程服务");
    let manager = get_service_manager();
    manager.uninstall()
}

/// 启动守护进程服务
#[tauri::command]
pub async fn start_daemon_service() -> Result<(), String> {
    crate::log_info!("启动守护进程服务");
    let manager = get_service_manager();
    manager.start()
}

/// 停止守护进程服务
#[tauri::command]
pub async fn stop_daemon_service() -> Result<(), String> {
    crate::log_info!("停止守护进程服务");
    let manager = get_service_manager();
    manager.stop()
}

/// 检查守护进程服务是否已安装
#[tauri::command]
pub async fn is_daemon_service_installed() -> Result<bool, String> {
    let manager = get_service_manager();
    Ok(manager.is_installed())
}

/// 检查守护进程服务是否正在运行
#[tauri::command]
pub async fn is_daemon_service_running() -> Result<bool, String> {
    let manager = get_service_manager();
    Ok(manager.is_running())
}

/// 获取守护进程日志
#[tauri::command]
pub async fn get_daemon_logs(level: Option<String>) -> Result<Vec<crate::log::LogEntry>, String> {
    crate::log_debug!("获取守护进程日志");
    let client = IpcClient::get_global();
    match client.get_daemon_logs(level.as_deref()).await {
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

/// 检查 EasyTier 二进制
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

/// 关闭守护进程
#[tauri::command]
pub async fn shutdown_daemon() -> Result<(), String> {
    crate::log_info!("关闭守护进程");
    let client = IpcClient::get_global();
    match client.shutdown().await {
        Ok(IpcResponse::Ok { .. }) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
    }
}
