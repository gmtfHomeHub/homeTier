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
    space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<AppRow, String> {
    // 校验权限：仅空间创建者可添加
    let caller_id = db.get_user_id()?.unwrap_or_default();
    crate::log_info!(format!("add_app: space_id={}", space_id));
    space_manager.check_owner(&space_id).await?;

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
    _space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    crate::log_info!(format!("update_app: app_id={}", app_id));

    // 校验权限：仅应用创建者可修改
    // 先查询应用所属 space
    let caller_id = db.get_user_id()?.unwrap_or_default();
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
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    crate::log_info!(format!("delete_app: app_id={}", app_id));
    let caller_id = db.get_user_id()?.unwrap_or_default();
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

/// 分享应用到其他空间（授权：目标空间成员即可使用该应用）
///
/// 源空间创建者可将应用复制到目标空间，目标空间无需重新配置。
#[tauri::command]
pub async fn share_app(
    app_id: String,
    target_space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<AppRow, String> {
    crate::log_info!(format!(
        "share_app: app_id={} -> space_id={}",
        app_id, target_space_id
    ));

    // 校验权限：仅源空间创建者可分享
    let caller_id = db.get_user_id()?.unwrap_or_default();
    let apps = db.list_apps_by_created(&app_id, &caller_id)?;
    if apps.is_empty() {
        return Err("无权限分享或应用不存在".to_string());
    }
    let source = &apps[0];

    // 校验目标空间存在
    let spaces = space_manager.list().await?;
    if !spaces.iter().any(|s| s.id.to_string() == target_space_id) {
        return Err("目标空间不存在".to_string());
    }

    // 目标空间已存在同名同源应用则跳过
    let existing = db.list_apps(&target_space_id)?;
    if existing.iter().any(|a| a.name == source.name) {
        return Err("目标空间已存在同名应用".to_string());
    }

    let app = AppRow {
        id: uuid::Uuid::new_v4().to_string(),
        space_id: target_space_id,
        name: source.name.clone(),
        category: source.category.clone(),
        icon: source.icon.clone(),
        protocol: source.protocol.clone(),
        hostname: source.hostname.clone(),
        port: source.port.clone(),
        pathname: source.pathname.clone(),
        sort_order: source.sort_order,
        created_by: caller_id,
        created_at: chrono::Local::now().to_rfc3339(),
    };
    db.insert_app(&app)?;
    crate::log_info!(format!("应用已分享: id={}", app.id));
    Ok(app)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
pub async fn get_system_apps(
    app: tauri::AppHandle,
) -> Result<Vec<crate::server::system_apps::SystemApp>, String> {
    use tauri::Manager;
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(crate::server::system_apps::load_system_apps(&data_dir))
}