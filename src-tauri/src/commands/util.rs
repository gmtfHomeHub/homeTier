use tauri::State;
use crate::db::Database;
use crate::platform;
use crate::types::{AuthResult, TunStatus};
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

#[tauri::command]
pub fn get_webapp_mode(db: State<'_, Arc<Database>>) -> Result<String, String> {
    Ok(db.get_setting("WEBAPP_MODE")?.unwrap_or_else(|| "iframe".to_string()))
}

#[tauri::command]
pub fn set_webapp_mode(mode: String, db: State<'_, Arc<Database>>) -> Result<(), String> {
    crate::log_info!(format!("设置 WebApp 模式: {}", mode));
    db.set_setting("WEBAPP_MODE", &mode)
}

/// 获取 TUN 设备状态
#[tauri::command]
pub fn get_tun_status() -> TunStatus {
    let adapter = platform::get_adapter();
    TunStatus {
        tun_available: platform::is_tun_available(),
        platform: adapter.get_platform_name(),
        elevated: adapter.is_elevated(),
    }
}

/// 重新检查 TUN 可用性（刷新缓存）
#[tauri::command]
pub fn refresh_tun_status() -> TunStatus {
    crate::log_info!("刷新 TUN 状态缓存");
    let available = platform::check_tun_available();
    let adapter = platform::get_adapter();
    TunStatus {
        tun_available: available,
        platform: adapter.get_platform_name(),
        elevated: adapter.is_elevated(),
    }
}

/// 由用户手动触发 TUN 授权。按平台不同会弹系统级授权对话框。
#[tauri::command]
pub fn authorize_tun() -> AuthResult {
    crate::log_info!("手动触发 TUN 授权");
    let result = platform::get_adapter().authorize_tun();
    if result.success && !result.needs_restart {
        // 授权后立即生效的平台（如 macOS），刷新缓存
        platform::init_tun_cap_check();
    }
    result
}