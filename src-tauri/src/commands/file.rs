use tauri::State;
use crate::types::{FileInfo, TransferProgress};
use crate::file::transfer::FileTransferManager;
use crate::space::manager::SpaceManager;
use crate::db::Database;
use std::sync::Arc;

#[tauri::command]
pub async fn send_file(
    space_id: String,
    file_path: String,
    password: Option<String>,
    file_manager: State<'_, Arc<FileTransferManager>>,
    space_manager: State<'_, Arc<SpaceManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<FileInfo, String> {
    let space_uuid = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    let sender_id = space_uuid;
    let path = std::path::PathBuf::from(&file_path);

    // 获取 peer 列表
    let peers = space_manager.get_peers_for_file_transfer(&space_uuid).await?;

    if peers.is_empty() {
        return Err("没有可用的 peers".to_string());
    }

    // 选择第一个 peer
    let (target_ip, target_port) = &peers[0];

    crate::log_info!(format!("发送文件: {} -> {}:{}", file_path, target_ip, target_port), &space_id);

    // 执行文件传输
    let file_info = file_manager.send_file(
        space_uuid,
        sender_id,
        path.clone(),
        password,
        target_ip,
        *target_port,
    ).await?;

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

    Ok(file_info)
}

#[tauri::command]
pub async fn receive_file(
    file_id: String,
    save_path: String,
    password: Option<String>,
    file_manager: State<'_, Arc<FileTransferManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&file_id).map_err(|e| e.to_string())?;

    // 从远程节点下载文件
    // 通过 EasyTier 虚拟网络连接到文件发送方
    crate::log_info!(format!("接收文件: id={}, save_path={}", file_id, save_path));
    file_manager.receive_file(id, save_path, password).await
}

#[tauri::command]
pub async fn list_files(
    space_id: String,
    limit: Option<u32>,
    db: State<'_, Arc<Database>>,
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