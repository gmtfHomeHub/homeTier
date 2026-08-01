use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::db::Database;
use crate::file::server::FileServer;
use crate::space::manager::SpaceManager;

/// 文件服务器注册表：每个连接中的空间一个 FileServer 实例，
/// 监听虚拟网端口 `19000 + (space_id % 1000)`。
pub struct FileServerRegistry {
    servers: Arc<RwLock<HashMap<Uuid, Arc<FileServer>>>>,
    storage_dir: PathBuf,
    db: Arc<Database>,
}

impl FileServerRegistry {
    pub fn new(storage_dir: PathBuf, db: Arc<Database>) -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            storage_dir,
            db,
        }
    }

    fn port_for(space_id: &Uuid) -> u16 {
        let base = crate::config::get_u16(crate::config::KEY_FILE_SERVER_PORT_BASE, crate::config::DEFAULT_FILE_SERVER_PORT_BASE);
        base + (space_id.as_u128() % 1000) as u16
    }

    pub fn db(&self) -> Arc<Database> {
        self.db.clone()
    }

    pub fn storage_dir(&self) -> PathBuf {
        self.storage_dir.clone()
    }

    /// 获取某空间的 FileServer（若未启动则自动创建并启动）
    pub async fn get_or_start(&self, space_id: &Uuid) -> Result<Arc<FileServer>, String> {
        {
            let servers = self.servers.read().await;
            if let Some(fs) = servers.get(space_id) {
                return Ok(fs.clone());
            }
        }

        let mut servers = self.servers.write().await;
        if let Some(fs) = servers.get(space_id) {
            return Ok(fs.clone());
        }

        let port = Self::port_for(space_id);
        let dir = self.storage_dir.join(space_id.to_string());
        let mut fs = FileServer::new(*space_id, dir);
        fs.start(port).await?;
        let fs = Arc::new(fs);
        servers.insert(*space_id, fs.clone());
        Ok(fs)
    }

    /// 获取某空间的 FileServer（不自动创建）
    pub async fn get(&self, space_id: &Uuid) -> Option<Arc<FileServer>> {
        self.servers.read().await.get(space_id).cloned()
    }

    /// 停止某空间的 FileServer
    pub async fn stop(&self, space_id: &Uuid) {
        if let Some(fs) = self.servers.write().await.remove(space_id) {
            fs.stop().await;
        }
    }

    /// 后台同步任务：确保所有已连接空间的 FileServer 在运行，已断开的停止。
    pub async fn sync(&self, space_manager: &Arc<SpaceManager>) {
        let spaces = match space_manager.list().await {
            Ok(s) => s,
            Err(_) => return,
        };

        for space in &spaces {
            if space.status == crate::types::SpaceStatus::Connected {
                if let Err(e) = self.get_or_start(&space.id).await {
                    crate::log_warn!(
                        format!("启动文件服务器失败: space_id={}, err={}", space.id, e),
                        &space.id.to_string()
                    );
                }
            } else {
                self.stop(&space.id).await;
            }
        }
    }
}
