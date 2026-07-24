use tauri::State;
use uuid::Uuid;
use crate::types::NetworkConfig;
use crate::space::manager::SpaceManager;
use std::sync::Arc;

#[tauri::command]
pub async fn get_effective_config(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<NetworkConfig, String> {
    let uuid = Uuid::parse_str(&space_id).map_err(|e| format!("无效的空间ID: {}", e))?;
    space_manager.get_effective_config(&uuid).await
}