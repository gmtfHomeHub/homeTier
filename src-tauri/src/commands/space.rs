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
    network_name: String,
    network_secret: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Space, String> {
    space_manager.join(network_name, network_secret).await
}

#[tauri::command]
pub async fn leave_space(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    space_manager.leave(&id).await
}

#[tauri::command]
pub async fn delete_space(
    space_id: String,
    caller_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    space_manager.delete(&id, &caller_id).await
}

#[tauri::command]
pub async fn list_spaces(
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Vec<Space>, String> {
    space_manager.list().await
}

#[tauri::command]
pub async fn remove_member(
    space_id: String,
    target_member_id: String,
    caller_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    space_manager.remove_member(&id, &target_member_id, &caller_id).await
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
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<String, String> {
    let spaces = space_manager.list().await?;
    let space = spaces.iter()
        .find(|s| s.id.to_string() == space_id)
        .ok_or_else(|| "Space not found".to_string())?;
    Ok(space_manager.generate_share_link(space))
}

#[tauri::command]
pub async fn parse_share_link(
    link: String,
) -> Result<ShareInfo, String> {
    SpaceManager::parse_share_link(&link)
}

#[tauri::command]
pub async fn connect_space(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    space_manager.connect(&id).await
}

#[tauri::command]
pub async fn disconnect_space(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    space_manager.disconnect(&id).await
}