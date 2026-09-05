use tauri::State;
use crate::db::Database;
use crate::db::models::AppRow;
use crate::space::manager::SpaceManager;
use crate::qr;
use serde::{Deserialize, Serialize};
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

#[tauri::command]
pub async fn get_system_apps(
    app: tauri::AppHandle,
) -> Result<Vec<crate::system_apps::SystemApp>, String> {
    use tauri::Manager;
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(crate::system_apps::load_system_apps(&data_dir))
}

// ==================== 应用分享（e=a_a） ====================

/// 导入用的应用数据（不含 id/space_id/created_* 等运行时字段）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppImport {
    name: String,
    category: Option<String>,
    icon: Option<String>,
    protocol: Option<String>,
    hostname: Option<String>,
    port: Option<String>,
    pathname: Option<String>,
}

/// 目标节点标识（用于接收端匹配空间）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PeerTarget {
    peer_id: u32,
    virtual_ip: Option<String>,
}

/// 加密载荷：name + network_name + apps + target_peers
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddAppPayload {
    name: String,
    network_name: String,
    apps: Vec<AppImport>,
    target_peers: Vec<PeerTarget>,
}

/// 导入结果
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAddAppsResult {
    space_id: String,
    space_name: String,
    imported: usize,
}

/// 生成应用分享链接（e=a_a）
#[tauri::command]
pub async fn generate_add_app_link(
    space_id: String,
    app_ids: Vec<String>,
    target_peer_ids: Vec<u32>,
    space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<String, String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    
    // 1. 获取空间基础信息
    let spaces = space_manager.spaces.read().await;
    let space = spaces.iter().find(|s| &s.id == &id)
        .ok_or_else(|| "Space not found".to_string())?;
    let name = space.name.clone();
    drop(spaces);

    // 2. 从 config_json 取 network_name
    let effective = space_manager.get_effective_config(&id).await?;
    let network_name = effective.network_name;

    // 3. 取选中的应用，映射为 AppImport
    let all_apps = db.list_apps(&space_id)?;
    let selected: Vec<AppImport> = all_apps
        .into_iter()
        .filter(|a| app_ids.contains(&a.id))
        .map(|a| AppImport {
            name: a.name,
            category: a.category,
            icon: a.icon,
            protocol: a.protocol,
            hostname: a.hostname,
            port: a.port,
            pathname: a.pathname,
        })
        .collect();
    if selected.is_empty() {
        return Err("未选择有效应用".to_string());
    }

    // 4. 取目标节点信息（peer_id + virtual_ip）
    let peers = space_manager.get_peers(&id).await?;
    let target_peers: Vec<PeerTarget> = peers
        .into_iter()
        .filter(|p| target_peer_ids.contains(&p.peer_id))
        .map(|p| PeerTarget {
            peer_id: p.peer_id,
            virtual_ip: p.virtual_ip,
        })
        .collect();

    // 5. 组装 payload 并加密
    let payload = AddAppPayload {
        name,
        network_name,
        apps: selected,
        target_peers,
    };
    let json = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    qr::encrypt_qr(qr::EVENT_ADD_APP, &json)
}

/// 扫码导入应用：base64 解码 → 解析 → 匹配空间 → 批量插入
#[tauri::command]
pub async fn import_add_apps(
    data: String,
    space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<ImportAddAppsResult, String> {
    // 1. base64 解码 + JSON 解析
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    let json_bytes = STANDARD.decode(&data).map_err(|e| format!("数据解码失败: {}", e))?;
    let payload: AddAppPayload = serde_json::from_slice(&json_bytes)
        .map_err(|e| format!("载荷解析失败: {}", e))?;

    // 2. 遍历本地空间，匹配 name + network_name + target_peers
    let spaces = space_manager.list().await?;
    let mut matched_space_id: Option<uuid::Uuid> = None;
    let mut matched_space_name = String::new();

    for space in spaces {
        if space.name != payload.name {
            continue;
        }
        // 解析 config_json 取 network_name
        let config_json = match space.config_json.as_ref() {
            Some(j) => j,
            None => continue,
        };
        let config = crate::easytier::config::NetworkConfig::from_config_json(config_json)
            .map_err(|e| format!("config_json 解析失败: {}", e))?;
        if config.network_name != payload.network_name {
            continue;
        }
        // 验证 target_peers：该空间当前在线 peer 是否包含所有 target peer_id
        let current_peers = space_manager.get_peers(&space.id).await.unwrap_or_default();
        let current_peer_ids: std::collections::HashSet<u32> = 
            current_peers.iter().map(|p| p.peer_id).collect();
        let all_targets_present = payload.target_peers.iter()
            .all(|tp| current_peer_ids.contains(&tp.peer_id));
        if !all_targets_present {
            continue;
        }
        matched_space_id = Some(space.id);
        matched_space_name = space.name.clone();
        break;
    }

    let space_id = matched_space_id.ok_or_else(|| 
        format!("未找到匹配空间：name={}, network_name={}", payload.name, payload.network_name)
    )?;

    // 3. 批量插入应用（跳过同名冲突）
    let mut imported = 0;
    for app_import in payload.apps {
        // 检查是否已存在同名应用
        let existing = db.list_apps(&space_id.to_string())?;
        if existing.iter().any(|a| a.name == app_import.name) {
            continue;
        }
        let app = AppRow {
            id: uuid::Uuid::new_v4().to_string(),
            space_id: space_id.to_string(),
            name: app_import.name,
            category: app_import.category,
            icon: app_import.icon,
            protocol: app_import.protocol.or(Some("http:".to_string())),
            hostname: app_import.hostname,
            port: app_import.port,
            pathname: app_import.pathname,
            sort_order: 0,
            created_by: db.get_user_id()?.unwrap_or_default(),
            created_at: chrono::Local::now().to_rfc3339(),
        };
        if db.insert_app(&app).is_ok() {
            imported += 1;
        }
    }

    crate::log_info!(format!("扫码导入应用完成：space={}, imported={}", matched_space_name, imported));
    Ok(ImportAddAppsResult {
        space_id: space_id.to_string(),
        space_name: matched_space_name,
        imported,
    })
}