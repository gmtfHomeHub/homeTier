use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::types::{FileInfo, TransferProgress, TransferStatus};
use crate::file::compress;
use crate::file::crypto;
use crate::file::server::FileServer;

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

    /// 发送文件（压缩 + 可选加密，流式上传到目标 FileServer）
    /// 传入 file_id 可复用同一文件 ID 广播到多个 peer。
    pub async fn send_file(
        &self,
        space_id: Uuid,
        sender_id: Uuid,
        file_path: PathBuf,
        password: Option<String>,
        target_ip: &str,
        target_port: u16,
        file_id: Option<Uuid>,
    ) -> Result<FileInfo, String> {
        let file_name = file_path.file_name()
            .ok_or_else(|| "Invalid file path".to_string())?
            .to_string_lossy()
            .to_string();

        let metadata = std::fs::metadata(&file_path)
            .map_err(|e| format!("File error: {}", e))?;
        let file_size = metadata.len();
        let file_id = file_id.unwrap_or_else(Uuid::new_v4);

        // 读取文件内容
        let mut data = std::fs::read(&file_path)
            .map_err(|e| format!("Read error: {}", e))?;

        // 压缩
        let compressed = compress::compress(&data, 3)
            .map_err(|e| format!("Compress error: {}", e))?;

        // 处理密码加密（如果有密码）
        let (body, encryption_used) = if let Some(ref pwd) = password {
            let enc = crypto::encrypt(&compressed, pwd)
                .map_err(|e| format!("Encrypt error: {}", e))?;
            (enc, true)
        } else {
            (compressed, password.is_some())
        };

        // 计算哈希（基于原始数据）
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
            is_compressed: true,
            is_password_protected: encryption_used,
            storage_path: None,
            created_at: chrono::Local::now(),
        };

        // 登记传输状态（以 file_id 作为传输标识，便于命令层返回）
        let transfer_id = file_id;
        self.transfers.write().await.insert(transfer_id, TransferState {
            file_name: file_name.clone(),
            total_bytes: body.len() as u64,
            bytes_transferred: 0,
            speed: 0,
            status: TransferStatus::Transferring,
            last_update: std::time::Instant::now(),
        });

        // 流式上传：分块推送并实时更新进度
        let sent = Arc::new(AtomicU64::new(0));
        let body_bytes = body.clone();
        let chunk_size = 64 * 1024;
        let chunks: Vec<bytes::Bytes> = body_bytes
            .chunks(chunk_size)
            .map(bytes::Bytes::copy_from_slice)
            .collect();

        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        let sent_probe = sent.clone();
        let transfers = self.transfers.clone();
        let t_id = transfer_id;
        let started = std::time::Instant::now();
        let total_bytes = body.len() as u64;

        // 后台进度更新任务
        tokio::spawn(async move {
            loop {
                let done = tokio::select! {
                    _ = &mut stop_rx => true,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => false,
                };
                let n = sent_probe.load(Ordering::SeqCst);
                let secs = started.elapsed().as_secs_f64().max(0.001);
                let speed = (n as f64 / secs) as u64;
                if let Some(state) = transfers.write().await.get_mut(&t_id) {
                    state.bytes_transferred = n;
                    state.speed = speed;
                    if n >= total_bytes {
                        state.status = TransferStatus::Completed;
                    }
                }
                if done {
                    break;
                }
            }
        });

        // 发送
        let url = format!("http://{}:{}/files/{}", target_ip, target_port, file_id);
        let client = reqwest::Client::new();

        use futures_util::stream::{self, StreamExt};
        let sent_stream = sent.clone();
        let body_stream = stream::iter(chunks.into_iter().map(move |c| {
            sent_stream.fetch_add(c.len() as u64, Ordering::SeqCst);
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(c)
        }));

        let send_result = client
            .put(&url)
            .body(reqwest::Body::wrap_stream(body_stream))
            .timeout(std::time::Duration::from_secs(300))
            .send()
            .await;

        let _ = stop_tx.send(());

        // 更新传输状态
        if let Some(state) = self.transfers.write().await.get_mut(&transfer_id) {
            match &send_result {
                Ok(resp) if resp.status().is_success() => {
                    state.status = TransferStatus::Completed;
                    state.bytes_transferred = total_bytes;
                }
                Ok(resp) => {
                    state.status = TransferStatus::Failed;
                    return Err(format!("上传失败: HTTP {}", resp.status()));
                }
                Err(e) => {
                    state.status = TransferStatus::Failed;
                    return Err(format!("上传失败: {}", e));
                }
            }
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

    /// 接收文件：从本地 FileServer 存储读取，解密解压，校验哈希后保存
    pub async fn receive_file(
        &self,
        file_server: &Arc<FileServer>,
        file_id: Uuid,
        save_path: String,
        password: Option<String>,
        expected_hash: Option<String>,
    ) -> Result<(), String> {
        let save_path = std::path::PathBuf::from(&save_path);
        if let Some(parent) = save_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {}", e))?;
        }

        crate::log_info!(format!("接收文件: id={}, save_path={}", file_id, save_path.display()));

        // 从 FileServer 存储读取（发送方已通过 HTTP PUT 落盘）
        let mut data = file_server.read_file(&file_id).await?;

        // 解密（如果提供了密码）
        if let Some(ref pwd) = password {
            data = crypto::decrypt(&data, pwd)
                .map_err(|e| format!("解密失败: {}", e))?;
        }

        // 解压缩
        data = compress::decompress(&data)
            .map_err(|e| format!("解压失败: {}", e))?;

        // 完整性校验（SHA256）
        if let Some(expected) = expected_hash {
            use sha2::{Sha256, Digest};
            let actual = hex::encode(Sha256::digest(&data));
            if actual != expected {
                return Err(format!("文件完整性校验失败 (hash mismatch)"));
            }
        }

        // 保存最终文件
        std::fs::write(&save_path, &data)
            .map_err(|e| format!("保存文件失败: {}", e))?;

        crate::log_info!(format!("文件保存成功: {}", save_path.display()));
        Ok(())
    }
}
