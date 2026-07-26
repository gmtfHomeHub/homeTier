use std::path::PathBuf;
use crate::daemon::{client::IpcClient, ipc::IpcResponse};
use crate::easytier::EasyTierManager;
use tauri::{Emitter, State};

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
    crate::easytier::github::fetch_available_versions().await
}

/// 升级 EasyTier（通过 daemon，无进度）
#[tauri::command]
pub async fn upgrade_easytier(version: String, source_path: Option<String>) -> Result<(), String> {
    let client = IpcClient::default_port();
    match client.upgrade(&version, source_path.as_deref()).await {
        Ok(IpcResponse::Ok { .. }) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Err(e) => Err(format!("连接 daemon 失败: {}", e)),
    }
}

/// 下载 EasyTier（带进度回调，直接在 App 上下文执行）
#[tauri::command]
pub async fn download_easytier_with_progress(
    version: String,
    app_handle: tauri::AppHandle,
    manager: State<'_, std::sync::Arc<EasyTierManager>>,
) -> Result<(), String> {
    // 在 App 上下文中下载，支持进度事件
    let path = manager.downloader.download_from_github(&version, Some(&app_handle)).await?;
    
    crate::log_info!(format!("[download_easytier_with_progress] 下载完成: {}", path.display()));
    
    // 可选：通知 daemon 重启运行中的实例以使用新版本
    // 这里仅返回成功，前端可选择调用 upgrade_easytier 重启
    Ok(())
}

/// 从源码编译 EasyTier 核心
#[tauri::command]
pub async fn build_easytier_from_source(
    manager: State<'_, std::sync::Arc<EasyTierManager>>,
) -> Result<String, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_dir = manifest_dir.join("..").join("third_libs").join("easytier");
    if !source_dir.exists() {
        return Err(format!(
            "EasyTier 源代码未找到: {}。请确保 third_libs/easytier/ 目录存在。",
            source_dir.display()
        ));
    }
    manager.downloader.build_from_source(&source_dir).await
}
