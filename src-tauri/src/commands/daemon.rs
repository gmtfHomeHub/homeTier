use crate::daemon::{client::IpcClient, service::get_service_manager};

/// 检查守护进程是否正在运行
#[tauri::command]
pub async fn check_daemon_running() -> Result<bool, String> {
    let client = IpcClient::new();
    Ok(client.ping())
}

/// 获取守护进程状态
#[tauri::command]
pub async fn get_daemon_status() -> Result<serde_json::Value, String> {
    let client = IpcClient::new();
    match client.get_status() {
        Ok(crate::daemon::ipc::IpcResponse::Ok { data }) => {
            Ok(data.unwrap_or(serde_json::Value::Null))
        }
        Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
        _ => Err("未知响应类型".into()),
    }
}

/// 连接到空间（通过守护进程）
#[tauri::command]
pub async fn daemon_connect_space(space_id: String) -> Result<(), String> {
    let client = IpcClient::new();
    match client.connect_space(&space_id) {
        Ok(crate::daemon::ipc::IpcResponse::Ok { .. }) => Ok(()),
        Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
        _ => Err("未知响应类型".into()),
    }
}

/// 断开空间连接（通过守护进程）
#[tauri::command]
pub async fn daemon_disconnect_space(space_id: String) -> Result<(), String> {
    let client = IpcClient::new();
    match client.disconnect_space(&space_id) {
        Ok(crate::daemon::ipc::IpcResponse::Ok { .. }) => Ok(()),
        Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
        _ => Err("未知响应类型".into()),
    }
}

/// 获取已连接的空间列表（通过守护进程）
#[tauri::command]
pub async fn daemon_list_spaces() -> Result<Vec<String>, String> {
    let client = IpcClient::new();
    match client.list_spaces() {
        Ok(crate::daemon::ipc::IpcResponse::Ok { data }) => {
            Ok(data.and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default())
        }
        Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
        _ => Err("未知响应类型".into()),
    }
}

/// 安装守护进程服务
#[tauri::command]
pub async fn install_daemon_service() -> Result<(), String> {
    let manager = get_service_manager();
    manager.install()
}

/// 卸载守护进程服务
#[tauri::command]
pub async fn uninstall_daemon_service() -> Result<(), String> {
    let manager = get_service_manager();
    manager.uninstall()
}

/// 启动守护进程服务
#[tauri::command]
pub async fn start_daemon_service() -> Result<(), String> {
    let manager = get_service_manager();
    manager.start()
}

/// 停止守护进程服务
#[tauri::command]
pub async fn stop_daemon_service() -> Result<(), String> {
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

/// 关闭守护进程
#[tauri::command]
pub async fn shutdown_daemon() -> Result<(), String> {
    let client = IpcClient::new();
    match client.shutdown() {
        Ok(crate::daemon::ipc::IpcResponse::Ok { .. }) => Ok(()),
        Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(e),
        _ => Err("未知响应类型".into()),
    }
}

/// Trait for pipe-like syntax
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

impl<T> Pipe for T {}
