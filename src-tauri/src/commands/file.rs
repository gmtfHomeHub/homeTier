use tauri::State;
use serde::Serialize;
use crate::types::{FileInfo, TransferProgress};
use crate::file::transfer::FileTransferManager;
use crate::file::registry::FileServerRegistry;
use crate::space::manager::SpaceManager;
use crate::db::Database;
use std::sync::Arc;

/// send_file 的返回：transfer_id 用于查询进度，file_info 用于列表
#[derive(Serialize)]
pub struct SendFileResult {
    pub transfer_id: String,
    pub file_info: FileInfo,
}

#[tauri::command]
pub async fn send_file(
    space_id: String,
    file_path: String,
    password: Option<String>,
    file_manager: State<'_, Arc<FileTransferManager>>,
    space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<SendFileResult, String> {
    let space_uuid = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    let sender_id = space_uuid;
    let path = std::path::PathBuf::from(&file_path);

    // 获取 peer 列表（虚拟 IP + 文件服务器端口）
    let peers = space_manager.get_peers_for_file_transfer(&space_uuid).await?;

    // 离线场景：无在线 peer 时创建空记录（接收方上线后通过信令 + HTTP 下载）
    if peers.is_empty() {
        let file_id = uuid::Uuid::new_v4();
        let file_name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let meta = std::fs::metadata(&path).map_err(|e| format!("无法读取文件: {}", e))?;
        let file_info = FileInfo {
            id: file_id,
            space_id: space_uuid,
            sender_id,
            file_name: file_name.clone(),
            file_size: meta.len(),
            file_hash: None,
            mime_type: None,
            is_compressed: false,
            is_password_protected: password.is_some(),
            storage_path: None,
            created_at: chrono::Local::now(),
        };
        let row = crate::db::models::FileRow {
            id: file_info.id.to_string(),
            space_id: file_info.space_id.to_string(),
            sender_id: file_info.sender_id.to_string(),
            file_name: file_info.file_name.clone(),
            file_size: file_info.file_size as i64,
            file_hash: file_info.file_hash.clone(),
            mime_type: file_info.mime_type.clone(),
            is_compressed: file_info.is_compressed,
            is_password_protected: file_info.is_password_protected,
            storage_path: file_info.storage_path.clone(),
            created_at: file_info.created_at.to_rfc3339(),
        };
        db.insert_file(&row)?;
        crate::log_info!(format!("空间无在线成员，文件已记录待离线接收: {}", file_name), &space_id);
        return Ok(SendFileResult { transfer_id: file_id.to_string(), file_info });
    }

    // 发送给所有在线成员（复用同一 file_id）
    let mut last_file_info = None;
    let mut shared_file_id: Option<uuid::Uuid> = None;
    for (target_ip, target_port) in &peers {
        crate::log_info!(format!("发送文件: {} -> {}:{}", file_path, target_ip, target_port), &space_id);

        let file_info = file_manager.send_file(
            space_uuid,
            sender_id,
            path.clone(),
            password.clone(),
            target_ip,
            *target_port,
            shared_file_id,
        ).await?;

        shared_file_id = Some(file_info.id);
        last_file_info = Some(file_info);
    }

    let file_info = last_file_info.ok_or_else(|| "文件发送失败".to_string())?;

    // 保存到数据库
    let row = crate::db::models::FileRow {
        id: file_info.id.to_string(),
        space_id: file_info.space_id.to_string(),
        sender_id: file_info.sender_id.to_string(),
        file_name: file_info.file_name.clone(),
        file_size: file_info.file_size as i64,
        file_hash: file_info.file_hash.clone(),
        mime_type: file_info.mime_type.clone(),
        is_compressed: file_info.is_compressed,
        is_password_protected: file_info.is_password_protected,
        storage_path: file_info.storage_path.clone(),
        created_at: file_info.created_at.to_rfc3339(),
    };
    db.insert_file(&row)?;

    Ok(SendFileResult {
        transfer_id: file_info.id.to_string(),
        file_info,
    })
}

#[tauri::command]
pub async fn receive_file(
    space_id: String,
    file_id: String,
    save_path: String,
    password: Option<String>,
    file_manager: State<'_, Arc<FileTransferManager>>,
    file_registry: State<'_, Arc<FileServerRegistry>>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let space_uuid = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::parse_str(&file_id).map_err(|e| e.to_string())?;

    // 获取该空间的 FileServer（本地存储的接收文件）
    let file_server = file_registry.get_or_start(&space_uuid).await?;

    // 从数据库查询文件哈希用于完整性校验
    let expected_hash = db.get_file(&space_id, &file_id)?.and_then(|f| f.file_hash);

    crate::log_info!(format!("接收文件: id={}, save_path={}", file_id, save_path));
    file_manager.receive_file(&file_server, id, save_path, password, expected_hash).await
}

/// 接收端记录收到的新文件（由前端收到 file 信令后调用）
#[tauri::command]
pub async fn record_received_file(
    file: crate::db::models::FileRow,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.insert_file(&file)
}

#[tauri::command]
pub async fn delete_file(
    space_id: String,
    file_id: String,
    file_registry: State<'_, Arc<FileServerRegistry>>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.delete_file(&space_id, &file_id)?;

    // 删除本地存储的 .bin 文件
    if let Ok(space_uuid) = uuid::Uuid::parse_str(&space_id) {
        if let Some(fs) = file_registry.get(&space_uuid).await {
            let _ = fs.delete_file(&uuid::Uuid::parse_str(&file_id).unwrap_or_default()).await;
        }
    }

    crate::log_info!(format!("删除文件: space_id={}, file_id={}", space_id, file_id));
    Ok(())
}

#[tauri::command]
pub async fn list_files(
    space_id: String,
    limit: Option<u32>,
    _db: State<'_, Arc<Database>>,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Vec<FileInfo>, String> {
    // 从数据库查询
    let rows = space_manager.list_space_files(&space_id, limit).await?;

    crate::log_debug!(format!("列出文件: space_id={}, count={}", space_id, rows.len()));

    let files = rows.iter().map(|r| {
        FileInfo {
            id: r.id.parse().unwrap_or_default(),
            space_id: r.space_id.parse().unwrap_or_default(),
            sender_id: r.sender_id.parse().unwrap_or_default(),
            file_name: r.file_name.clone(),
            file_size: r.file_size as u64,
            file_hash: r.file_hash.clone(),
            mime_type: r.mime_type.clone(),
            is_compressed: r.is_compressed,
            is_password_protected: r.is_password_protected,
            storage_path: r.storage_path.clone(),
            created_at: chrono::DateTime::parse_from_rfc3339(&r.created_at)
                .map(|d| d.with_timezone(&chrono::Local))
                .unwrap_or_else(|_| chrono::Local::now()),
        }
    }).collect();

    Ok(files)
}

#[tauri::command]
pub async fn get_transfer_progress(
    transfer_id: String,
    file_manager: State<'_, Arc<FileTransferManager>>,
) -> Result<Option<TransferProgress>, String> {
    let id = uuid::Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?;
    let progress = file_manager.get_progress(&id).await;
    crate::log_debug!(format!("查询传输进度: transfer_id={}, has_progress={}", transfer_id, progress.is_some()));
    Ok(progress)
}
