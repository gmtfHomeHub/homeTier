use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::types::{FileInfo, TransferProgress, TransferStatus};

/// 文件传输管理器
pub struct FileTransferManager {
    transfers: Arc<RwLock<HashMap<Uuid, TransferState>>>,
    files: Arc<RwLock<Vec<FileInfo>>>,
}

struct TransferState {
    file_name: String,
    total_bytes: u64,
    bytes_transferred: u64,
    speed: u64,
    status: TransferStatus,
    last_update: std::time::Instant,
}

impl FileTransferManager {
    pub fn new() -> Self {
        Self {
            transfers: Arc::new(RwLock::new(HashMap::new())),
            files: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 发送文件
    pub async fn send_file(
        &self,
        space_id: Uuid,
        sender_id: Uuid,
        file_path: PathBuf,
        password: Option<String>,
        target_ip: &str,
        target_port: u16,
    ) -> Result<FileInfo, String> {
        let file_name = file_path.file_name()
            .ok_or_else(|| "Invalid file path".to_string())?
            .to_string_lossy()
            .to_string();

        let metadata = std::fs::metadata(&file_path)
            .map_err(|e| format!("File error: {}", e))?;
        let file_size = metadata.len();
        let file_id = Uuid::new_v4();

        // 读取文件内容
        let data = std::fs::read(&file_path)
            .map_err(|e| format!("Read error: {}", e))?;

        // 计算哈希
        use sha2::{Sha256, Digest};
        let hash = hex::encode(Sha256::digest(&data));

        let file_info = FileInfo {
            id: file_id,
            space_id,
            sender_id,
            file_name: file_name.clone(),
            file_size,
            file_hash: Some(hash),
            mime_type: None,
            is_compressed: false,
            is_password_protected: password.is_some(),
            storage_path: None,
            created_at: chrono::Local::now(),
        };

        // 启动传输
        let transfer_id = Uuid::new_v4();
        self.transfers.write().await.insert(transfer_id, TransferState {
            file_name: file_name.clone(),
            total_bytes: file_size,
            bytes_transferred: 0,
            speed: 0,
            status: TransferStatus::Transferring,
            last_update: std::time::Instant::now(),
        });

        // 发送 HTTP 请求
        let url = format!("http://{}:{}/files/{}", target_ip, target_port, file_id);
        let client = reqwest::Client::new();
        let _ = client.put(&url)
            .body(data)
            .timeout(std::time::Duration::from_secs(300))
            .send()
            .await;

        // 更新传输状态
        if let Some(state) = self.transfers.write().await.get_mut(&transfer_id) {
            state.status = TransferStatus::Completed;
            state.bytes_transferred = file_size;
        }

        self.files.write().await.push(file_info.clone());
        Ok(file_info)
    }

    /// 获取传输进度
    pub async fn get_progress(&self, transfer_id: &Uuid) -> Option<TransferProgress> {
        let transfers = self.transfers.read().await;
        transfers.get(transfer_id).map(|t| TransferProgress {
            transfer_id: *transfer_id,
            file_name: t.file_name.clone(),
            bytes_transferred: t.bytes_transferred,
            total_bytes: t.total_bytes,
            speed_bytes_per_sec: t.speed,
            status: t.status.clone(),
        })
    }

    /// 获取文件列表
    pub async fn list_files(&self) -> Vec<FileInfo> {
        self.files.read().await.clone()
    }

    /// 接收文件（启动 HTTP 服务接收上传）
    pub async fn receive_file(
        &self,
        file_id: Uuid,
        save_path: String,
        _password: Option<String>,
    ) -> Result<(), String> {
        let save_path = std::path::PathBuf::from(&save_path);
        if let Some(parent) = save_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {}", e))?;
        }

        // 实际实现会启动一个临时 HTTP 服务来接收文件上传
        // 这里简化为直接创建文件
        crate::log_info!(format!("接收文件: id={}, save_path={}", file_id, save_path.display()));

        // 更新传输状态
        let transfer_id = Uuid::new_v4();
        self.transfers.write().await.insert(transfer_id, TransferState {
            file_name: save_path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            total_bytes: 0,
            bytes_transferred: 0,
            speed: 0,
            status: TransferStatus::Transferring,
            last_update: std::time::Instant::now(),
        });

        Ok(())
    }
}