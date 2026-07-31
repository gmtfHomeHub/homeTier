use tauri::State;
use crate::db::Database;
use std::sync::Arc;

#[tauri::command]
pub async fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn get_system_config(db: State<'_, Arc<Database>>) -> Result<Option<String>, String> {
    db.get_setting("easytier_system_config")
}

#[tauri::command]
pub fn set_system_config(config: String, db: State<'_, Arc<Database>>) -> Result<(), String> {
    crate::log_info!("更新系统配置");
    db.set_setting("easytier_system_config", &config)
}

#[tauri::command]
pub fn get_relay_prefix(db: State<'_, Arc<Database>>) -> Result<String, String> {
    Ok(db.get_setting("RELAY_NETWORK_PREFIX")?.unwrap_or_else(|| "homeTier_".to_string()))
}

#[tauri::command]
pub fn set_relay_prefix(prefix: String, db: State<'_, Arc<Database>>) -> Result<(), String> {
    crate::log_info!(format!("设置中继前缀: {}", prefix));
    db.set_setting("RELAY_NETWORK_PREFIX", &prefix)
}

/// 读取日志开关（默认开启）
#[tauri::command]
pub fn get_log_enabled(db: State<'_, Arc<Database>>) -> Result<bool, String> {
    Ok(db.get_setting("LOG_ENABLED")?.as_deref() != Some("0"))
}

/// 设置日志开关（写 DB + 设本地标志 + 同步 daemon）
#[tauri::command]
pub async fn set_log_enabled(enabled: bool, db: State<'_, Arc<Database>>) -> Result<(), String> {
    crate::log::set_log_enabled(enabled);
    db.set_setting("LOG_ENABLED", if enabled { "1" } else { "0" })?;
    // 同步 daemon 进程的日志开关
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let client = crate::daemon::client::IpcClient::get_global();
        if client.ping().await {
            let _ = client.set_log_enabled(enabled).await;
        }
    }
    crate::log_info!(format!("设置日志开关: {}", if enabled { "开启" } else { "关闭" }));
    Ok(())
}