use tauri::State;
use crate::db::Database;
use crate::db::models::AppRow;
use crate::space::manager::SpaceManager;
use std::sync::Arc;

#[tauri::command]
pub async fn add_app(
    space_id: String,
    name: String,
    category: Option<String>,
    icon: Option<String>,
    protocol: Option<String>,
    hostname: Option<String>,
    port: Option<String>,
    pathname: Option<String>,
    caller_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<AppRow, String> {
    // 校验权限：仅空间创建者可添加
    crate::log_info!(format!("add_app: space_id={}, caller_id={}", space_id, caller_id));
    space_manager.check_owner(&space_id, &caller_id).await?;

    let app = AppRow {
        id: uuid::Uuid::new_v4().to_string(),
        space_id,
        name,
        category,
        icon,
        protocol: protocol.or(Some("http:".to_string())),
        hostname,
        port,
        pathname,
        sort_order: 0,
        created_by: caller_id,
        created_at: chrono::Local::now().to_rfc3339(),
    };
    db.insert_app(&app)?;
    crate::log_info!(format!("应用已添加: id={}", app.id));
    Ok(app)
}

#[tauri::command]
pub async fn update_app(
    app_id: String,
    name: String,
    category: Option<String>,
    icon: Option<String>,
    protocol: Option<String>,
    hostname: Option<String>,
    port: Option<String>,
    pathname: Option<String>,
    caller_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    crate::log_info!(format!("update_app: app_id={}, caller_id={}", app_id, caller_id));

    // 校验权限：仅应用创建者可修改
    // 先查询应用所属 space
    let apps = db.list_apps_by_created(&app_id, &caller_id)?;
    if apps.is_empty() {
        return Err("无权限修改或应用不存在".to_string());
    }
    let existing = &apps[0];

    let app = AppRow {
        id: app_id,
        space_id: existing.space_id.clone(),
        name,
        category,
        icon,
        protocol: protocol.or(Some("http:".to_string())),
        hostname,
        port,
        pathname,
        sort_order: existing.sort_order,
        created_by: caller_id,
        created_at: existing.created_at.clone(),
    };
    db.update_app(&app)?;
    crate::log_info!("应用已更新");
    Ok(())
}

#[tauri::command]
pub async fn delete_app(
    app_id: String,
    caller_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    crate::log_info!(format!("delete_app: app_id={}, caller_id={}", app_id, caller_id));
    db.delete_app(&app_id, &caller_id)?;
    crate::log_info!("应用已删除");
    Ok(())
}

#[tauri::command]
pub async fn list_apps(
    space_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AppRow>, String> {
    db.list_apps(&space_id)
}