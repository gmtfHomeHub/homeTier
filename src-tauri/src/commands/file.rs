use tauri::State;
use crate::types::{FileInfo, TransferProgress};
use crate::file::transfer::FileTransferManager;
use crate::space::manager::SpaceManager;
use std::sync::Arc;

#[tauri::command]
pub async fn send_file(
    space_id: String,
    file_path: String,
    password: Option<String>,
    file_manager: State<'_, Arc<FileTransferManager>>,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<FileInfo, String> {
    let space_uuid = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    let sender_id = space_uuid;
    let path = std::path::PathBuf::from(&file_path);

    // 通过 EasyTier 获取目标节点的 IP 和端口
    // 在 P2P 网络中，通过虚拟 IP 直接连接
    // 实际使用时会从 EasyTier RPC 获取 peer 的虚拟 IP
    let target_ip = "127.0.0.1"; // 通过 EasyTier 虚拟网络获取 peer IP
    let target_port = 0; // 通过 EasyTier 服务发现获取端口

    crate::log_info!(format!("发送文件: {} -> {}:{}", file_path, target_ip, target_port), &space_id);

    file_manager.send_file(
        space_uuid,
        sender_id,
        path,
        password,
        target_ip,
        target_port,
    ).await
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
    file_manager: State<'_, Arc<FileTransferManager>>,
) -> Result<Vec<FileInfo>, String> {
    Ok(file_manager.list_files().await)
}

#[tauri::command]
pub async fn get_transfer_progress(
    transfer_id: String,
    file_manager: State<'_, Arc<FileTransferManager>>,
) -> Result<Option<TransferProgress>, String> {
    let id = uuid::Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?;
    Ok(file_manager.get_progress(&id).await)
}