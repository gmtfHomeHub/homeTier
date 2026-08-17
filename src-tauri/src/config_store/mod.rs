//! P2P 分布式配置文件存储服务
//!
//! 每个节点既是存储服务端（监听 TCP 9877）也是客户端，
//! 通过 EasyTier 虚拟局域网提供的 IP 可达性互相请求/推送配置文件。
//! 参考 docs/分布式配置文件存储服务设计文档.md。

pub mod client;
pub mod queue;
pub mod server;
pub mod store;

pub use store::{ConfigFile, ConfigFileMeta, ConfigStore, StoreError};

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 默认监听端口（设计文档约定 9877）
pub const DEFAULT_PORT: u16 = 9877;

/// 配置存储服务的统一入口
///
/// 内部组装：本地存储（store）+ 存储队列（queue）+ TCP 服务端（server）
pub struct ConfigStoreService {
    pub store: Arc<ConfigStore>,
    pub queue: Arc<queue::StoreQueue>,
}

/// 请求/存储协议消息（JSON 行协议，TCP 直连，默认端口 9877）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum Message {
    /// 请求某个配置文件的最新版本信息
    QueryVersion { name: String },
    /// 返回版本信息
    VersionInfo {
        name: String,
        version: u32,
        timestamp: u64,
        checksum: Option<String>,
    },
    /// 请求下载配置文件（from_version: 增量更新，预留）
    RequestFile {
        name: String,
        from_version: Option<u32>,
    },
    /// 返回文件内容
    FileResponse {
        name: String,
        version: u32,
        content: Vec<u8>,
        checksum: Option<String>,
    },
    /// 存储配置文件到目标节点（推式更新）
    StoreFile {
        name: String,
        version: u32,
        content: Vec<u8>,
        timestamp: u64,
        checksum: Option<String>,
    },
    /// 存储结果
    StoreAck {
        name: String,
        success: bool,
        error: Option<String>,
    },
}

impl Message {
    /// 序列化为 JSON 行（以 \n 结尾）
    pub fn to_line(&self) -> Vec<u8> {
        let mut line = serde_json::to_vec(self).unwrap_or_default();
        line.push(b'\n');
        line
    }

    /// 从一行 JSON 解析
    pub fn from_line(line: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(line)
    }
}

impl ConfigStoreService {
    /// 初始化服务（创建存储目录、组装队列，不含消费者任务）
    pub fn new(root: PathBuf) -> (Arc<Self>, mpsc::UnboundedReceiver<ConfigFile>) {
        let store = Arc::new(ConfigStore::new(root));
        let (queue, receiver) = queue::StoreQueue::new();
        (Arc::new(Self { store, queue }), receiver)
    }

    /// 启动队列消费者（必须在 Tokio 上下文中调用）
    pub fn start_consumer(self: &Arc<Self>, receiver: mpsc::UnboundedReceiver<ConfigFile>) {
        self.queue.start(Arc::clone(&self.store), receiver);
    }

    /// 本地存储一个配置文件（走队列，去重 + 文件锁保证一致性）
    pub fn store_local(&self, file: ConfigFile) {
        self.queue.submit(file);
    }

    /// 启动 TCP 监听（0.0.0.0:{port}），阻塞直到服务关闭
    pub async fn serve(self: Arc<Self>, port: u16) -> std::io::Result<()> {
        server::serve(self, port).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn queue_dedup() {
        let _store = Arc::new(ConfigStore::new(PathBuf::from("/tmp/homeTier-cs-test")));
        let (queue, _receiver) = queue::StoreQueue::new();
        let f1 = ConfigFile {
            name: "space_settings".into(),
            version: 1,
            content: b"v1".to_vec(),
            timestamp: 1,
            checksum: None,
        };
        let f2 = ConfigFile {
            name: "space_settings".into(),
            version: 2,
            content: b"v2".to_vec(),
            timestamp: 2,
            checksum: None,
        };
        queue.submit(f1);
        queue.submit(f2);
        // 队列应只保留最新任务
        assert_eq!(queue.pending_len(), 1);
    }

    #[test]
    fn version_conflict() {
        let root = PathBuf::from("/tmp/homeTier-cs-test2");
        let _ = std::fs::remove_dir_all(&root);
        let store = ConfigStore::new(root.clone());
        let f1 = ConfigFile {
            name: "cfg".into(),
            version: 2,
            content: b"v2".to_vec(),
            timestamp: 2,
            checksum: None,
        };
        store.store(f1).unwrap();
        let f0 = ConfigFile {
            name: "cfg".into(),
            version: 1,
            content: b"v1".to_vec(),
            timestamp: 1,
            checksum: None,
        };
        // 旧版本写入应被拒绝
        assert!(matches!(store.store(f0), Err(StoreError::VersionConflict)));
    }

    #[tokio::test]
    async fn tcp_roundtrip() {
        let root = PathBuf::from("/tmp/homeTier-cs-test3");
        let _ = std::fs::remove_dir_all(&root);
        let (service, receiver) = ConfigStoreService::new(root.clone());
        service.start_consumer(receiver);

        // 启动 TCP 服务
        let server = Arc::clone(&service);
        tokio::spawn(async move {
            let _ = server.serve(DEFAULT_PORT).await;
        });
        // 等待端口就绪
        for _ in 0..50 {
            if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", DEFAULT_PORT))
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        let remote = crate::config_store::client::RemoteStore::new("127.0.0.1", DEFAULT_PORT);

        // 空配置：版本为 None
        let meta = remote.query_version("space_settings").await.unwrap();
        assert!(meta.is_none());

        // 推送配置
        let f = ConfigFile {
            name: "space_settings".into(),
            version: 1,
            content: b"hello config".to_vec(),
            timestamp: 1700000000,
            checksum: None,
        };
        let ok = remote.store_file(&f).await.unwrap();
        assert!(ok);

        // 查询版本
        let meta = remote.query_version("space_settings").await.unwrap().unwrap();
        assert_eq!(meta.version, 1);
        assert_eq!(meta.timestamp, 1700000000);

        // 下载内容
        let file = remote.request_file("space_settings").await.unwrap().unwrap();
        assert_eq!(file.content, b"hello config".to_vec());
        assert_eq!(file.version, 1);

        // 版本回退：服务端返回 success（已入队），但队列消费时被丢弃，版本保持 1
        let f_old = ConfigFile {
            name: "space_settings".into(),
            version: 0,
            content: b"old".to_vec(),
            timestamp: 0,
            checksum: None,
        };
        let result = remote.store_file(&f_old).await;
        assert!(result.is_ok());
        // 等待队列消费完成
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let meta = remote.query_version("space_settings").await.unwrap().unwrap();
        assert_eq!(meta.version, 1, "版本回退应被丢弃，仍保持 v1");
    }
}
