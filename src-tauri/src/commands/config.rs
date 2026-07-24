use tauri::State;
use crate::types::NetworkConfig;
use crate::space::manager::SpaceManager;
use crate::db::Database;
use std::sync::Arc;
use serde_json::Value;

#[tauri::command]
pub async fn get_effective_config(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<NetworkConfig, String> {
    let config = space_manager.get_effective_config(&space_id).await?;
    Ok(serde_json::from_value(config).map_err(|e| format!("配置解析失败: {}", e))?)
}

#[tauri::command]
pub async fn update_local_config(
    space_id: String,
    local_config: Value,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    space_manager.update_local_config(&space_id, local_config).await
}