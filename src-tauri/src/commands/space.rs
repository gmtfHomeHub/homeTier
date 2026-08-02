use tauri::State;
use crate::types::{Space, ShareInfo, Member};
use crate::space::manager::SpaceManager;
use crate::db::Database;
use std::sync::Arc;

#[tauri::command]
pub async fn get_space_config(
    space_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<String>, String> {
    db.get_space_config(&space_id)
}

#[tauri::command]
pub async fn update_space_config(
    space_id: String,
    config_json: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.update_space_config(&space_id, &config_json)
}

#[tauri::command]
pub async fn create_space(
    name: String,
    network_secret: String,
    owner_id: String,
    description: Option<String>,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Space, String> {
    crate::log_info!(format!("命令: create_space name={}, owner_id={}", name, owner_id));
    space_manager.create(name, network_secret, owner_id, description).await
}

#[tauri::command]
pub async fn join_space(
    config_json: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Space, String> {
    crate::log_info!("命令: join_space");
    let config = serde_json::from_str::<crate::easytier::config::NetworkConfig>(&config_json)
        .map_err(|e| format!("配置 json 解析失败: {}", e))?;
    space_manager.join(config).await
}

#[tauri::command]
pub async fn leave_space(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    crate::log_info!(format!("离开空间: {}", space_id));
    space_manager.leave(&id).await
}

#[tauri::command]
pub async fn delete_space(
    space_id: String,
    caller_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    crate::log_info!(format!("删除空间: {}, caller={}", space_id, caller_id));
    space_manager.delete(&id, &caller_id).await
}

#[tauri::command]
pub async fn list_spaces(
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Vec<Space>, String> {
    space_manager.list().await
}

#[tauri::command]
pub async fn list_members(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Vec<Member>, String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    space_manager.list_members(&id).await
}

#[tauri::command]
pub async fn generate_share_link(
    space_id: String,
    ip: Option<String>,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<String, String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    space_manager.generate_share_link(&id, ip).await
}

#[tauri::command]
pub async fn parse_share_link(
    link: String,
) -> Result<ShareInfo, String> {
    crate::space::share::decrypt_share_link(&link)
}

#[tauri::command]
pub async fn connect_space(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    crate::log_info!(format!("连接空间: {}", space_id));
    space_manager.connect(&id).await
}

#[tauri::command]
pub async fn disconnect_space(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    crate::log_info!(format!("断开空间: {}", space_id));
    space_manager.disconnect(&id).await
}

#[tauri::command]
pub async fn get_space_status(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Option<serde_json::Value>, String> {
    space_manager.get_space_status(&space_id).await
}

#[tauri::command]
pub async fn patch_space_config(
    space_id: String,
    patch: serde_json::Value,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    space_manager.patch_config(&space_id, patch).await
}

#[tauri::command]
pub async fn update_local_config(
    space_id: String,
    config_json: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let config: crate::easytier::config::NetworkConfig = serde_json::from_str(&config_json)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    crate::log_info!(format!("更新本地配置: {}", space_id));
    space_manager.update_local_config(&id, config).await
}