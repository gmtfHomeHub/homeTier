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
    db.set_setting("easytier_system_config", &config)
}

#[tauri::command]
pub fn get_relay_prefix(db: State<'_, Arc<Database>>) -> Result<String, String> {
    Ok(db.get_setting("RELAY_NETWORK_PREFIX")?.unwrap_or_else(|| "homeTier_".to_string()))
}

#[tauri::command]
pub fn set_relay_prefix(prefix: String, db: State<'_, Arc<Database>>) -> Result<(), String> {
    db.set_setting("RELAY_NETWORK_PREFIX", &prefix)
}