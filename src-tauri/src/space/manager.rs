use std::sync::Arc;
use std::path::PathBuf;
use uuid::Uuid;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use std::collections::HashMap;
use crate::db::Database;
use crate::types::{Space, SpaceStatus, Member, ShareInfo};
use crate::db::models::SpaceRow;
use crate::chat::server::ChatServer;
use crate::chat::client::ChatClient;
use crate::voice::server::VoiceServer;
use crate::screen::server::ScreenShareSignalServer;
use crate::easytier::config::NetworkConfig;
use crate::file::FileServer;

/// 空间管理器
pub struct SpaceManager {
    db: Arc<Database>,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    easytier: Arc<crate::easytier::EasyTierManager>,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ipc_client: Arc<crate::daemon::client::IpcClient>,
    pub(crate) spaces: Arc<RwLock<Vec<Space>>>,
    /// 聊天服务器映射: space_id -> ChatServer
    pub(crate) chat_servers: Arc<RwLock<HashMap<Uuid, ChatServer>>>,
    /// 聊天客户端映射: space_id -> ChatClient
    chat_clients: Arc<RwLock<HashMap<Uuid, ChatClient>>>,
    /// 语音服务器映射: space_id -> VoiceServer
    voice_servers: Arc<RwLock<HashMap<Uuid, VoiceServer>>>,
    /// 屏幕共享服务器映射: space_id -> ScreenShareSignalServer
    screen_servers: Arc<RwLock<HashMap<Uuid, ScreenShareSignalServer>>>,
    /// 文件服务器映射: space_id -> FileServer
    file_servers: Arc<RwLock<HashMap<Uuid, FileServer>>>,
    /// 文件存储根目录
    storage_dir: Arc<RwLock<PathBuf>>,
    /// 取消令牌映射: space_id -> CancellationToken（用于取消 discover_and_connect_peers）
    cancel_tokens: Arc<RwLock<HashMap<Uuid, CancellationToken>>>,
    /// 连接任务句柄映射: space_id -> JoinHandle
    connect_handles: Arc<RwLock<HashMap<Uuid, tokio::task::JoinHandle<()>>>>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl Clone for SpaceManager {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            ipc_client: self.ipc_client.clone(),
            spaces: self.spaces.clone(),
            chat_servers: self.chat_servers.clone(),
            chat_clients: self.chat_clients.clone(),
            voice_servers: self.voice_servers.clone(),
            screen_servers: self.screen_servers.clone(),
            file_servers: self.file_servers.clone(),
            storage_dir: self.storage_dir.clone(),
            cancel_tokens: self.cancel_tokens.clone(),
            connect_handles: self.connect_handles.clone(),
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl SpaceManager {
    pub fn new(
        db: Arc<Database>,
        _easytier: Arc<crate::easytier::EasyTierManager>,
        ipc_client: Arc<crate::daemon::client::IpcClient>,
    ) -> Self {
        let storage_dir = std::env::var("APPDATA_DIR")
            .or_else(|_| std::env::var("HOME"))
            .map(|p| PathBuf::from(p).join("homeTier/files"))
            .unwrap_or_else(|_| PathBuf::from(".files"));

        let _ = std::fs::create_dir_all(&storage_dir);

Self {
            db,
            ipc_client,
            spaces: Arc::new(RwLock::new(Vec::new())),
            chat_servers: Arc::new(RwLock::new(HashMap::new())),
            chat_clients: Arc::new(RwLock::new(HashMap::new())),
            voice_servers: Arc::new(RwLock::new(HashMap::new())),
            screen_servers: Arc::new(RwLock::new(HashMap::new())),
            file_servers: Arc::new(RwLock::new(HashMap::new())),
            storage_dir: Arc::new(RwLock::new(storage_dir)),
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
            connect_handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建空间（创建者自动成为 owner）
    pub async fn create(&self, name: String, network_secret: String, description: Option<String>) -> Result<Space, String> {
        let space_id = Uuid::new_v4();
        let owner_uuid = self.db.get_user_id()?.unwrap_or_else(|| "local-user".to_string());
        let network_name = name.clone();

        let space = Space {
            id: space_id,
            name,
            description,
            owner_id: Some(owner_uuid.clone()),
            network_name: network_name.clone(),
            network_secret: network_secret.clone(),
            created_at: chrono::Local::now(),
            last_connected_at: None,
            is_auto_connect: false,
            status: SpaceStatus::Disconnected,
            virtual_ip: None,
            member_count: 1,
            config_json: None,
        };

        let row = SpaceRow {
            id: space.id.to_string(),
            name: space.name.clone(),
            owner_id: space.owner_id.clone(),
            network_name: space.network_name.clone(),
            network_secret: space.network_secret.clone(),
            description: space.description.clone(),
            created_at: space.created_at.to_rfc3339(),
            last_connected_at: None,
            is_auto_connect: false,
                config_json: None,
        };
        self.db.insert_space(&row)?;
        self.db.add_member(&space_id.to_string(), &owner_uuid, &space.name, true)?;
        self.spaces.write().await.push(space.clone());

        crate::log_info!(format!("创建空间: {} (id={}, owner={})", space.name, space.id, owner_uuid), &space.id.to_string());
        Ok(space)
    }

    /// 加入空间
    /// config 为前端传入的 easytier 配置 json 反序列化结果，缺省字段已由 serde(default) 补全，
    /// 完整配置序列化后写入 config_json 落库
    pub async fn join(&self, config: NetworkConfig) -> Result<Space, String> {
        let network_name = config.network_name.clone();
        let network_secret = config.network_secret.clone();
        let space = Space {
            id: Uuid::new_v4(),
            name: network_name.clone(),
            description: None,
            owner_id: None,
            network_name: network_name.clone(),
            network_secret: network_secret.clone(),
            created_at: chrono::Local::now(),
            last_connected_at: None,
            is_auto_connect: false,
            status: SpaceStatus::Disconnected,
            virtual_ip: None,
            member_count: 1,
            config_json: None,
        };

        let row = SpaceRow {
            id: space.id.to_string(),
            name: space.name.clone(),
            owner_id: None,
            network_name: space.network_name.clone(),
            network_secret: space.network_secret.clone(),
            description: None,
            created_at: space.created_at.to_rfc3339(),
            last_connected_at: None,
            is_auto_connect: false,
                config_json: None,
        };
        self.db.insert_space(&row)?;

        // 完整配置 json 落库（默认值已由后端 serde(default) 补全）
        let config_json = serde_json::to_string(&config)
            .map_err(|e| format!("序列化配置失败: {}", e))?;
        self.db.update_space_config(&space.id.to_string(), &config_json)?;
        crate::log_info!(
            format!("加入空间: 配置已写入 config json (dhcp={}, ip={}, peers={}, listeners={})",
                config.dhcp,
                config.virtual_ipv4,
                config.peer_urls.len(),
                config.listener_urls.len()),
            &space.id.to_string()
        );

        self.spaces.write().await.push(space.clone());
        crate::log_info!(format!("加入空间: {}", space.name), &space.id.to_string());
        Ok(space)
    }

    /// 离开空间
    pub async fn leave(&self, space_id: &Uuid) -> Result<(), String> {
        self.disconnect(space_id).await?;
        crate::log_info!(format!("离开空间: {}", space_id), &space_id.to_string());
        Ok(())
    }

    /// 删除空间
    pub async fn delete(&self, space_id: &Uuid) -> Result<(), String> {
        let caller_id = self.db.get_user_id()?.unwrap_or_default();
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id.as_str()) {
            return Err("只有空间所有者才能删除空间".to_string());
        }
        drop(spaces);

        self.leave(space_id).await?;
        self.db.delete_space(&space_id.to_string())?;
        self.spaces.write().await.retain(|s| s.id != *space_id);
        crate::log_info!(format!("空间已删除: {}", space_id), &space_id.to_string());
        Ok(())
    }

    /// 获取空间列表
    pub async fn list(&self) -> Result<Vec<Space>, String> {
        let rows = self.db.list_spaces()?;
        let mut spaces = Vec::new();

        // 查询 daemon 获取运行中的 space 列表
        let running_spaces: Vec<String> = match self.ipc_client.list_spaces().await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { data }) => {
                data.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
            }
            _ => Vec::new(),
        };

        for row in rows {
            let id: Uuid = row.id.parse().unwrap_or_default();
            let is_running = running_spaces.contains(&row.id);
            let status = if is_running { SpaceStatus::Connected } else { SpaceStatus::Disconnected };

            // 通过 RPC 查询运行时状态
            let (member_count, virtual_ip) = if is_running {
                match self.ipc_client.get_space_status(&row.id).await {
                    Ok(crate::daemon::ipc::IpcResponse::Ok { data }) => {
                        if let Some(status_val) = data {
                            let peer_count = status_val.get("connected_peers")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            let vip = status_val.get("virtual_ip")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            (peer_count + 1, vip)
                        } else {
                            (0, None)
                        }
                    }
                    _ => (0, None),
                }
            } else {
                (0, None)
            };

            spaces.push(Space {
                id,
                name: row.name,
                description: row.description,
                owner_id: row.owner_id,
                network_name: row.network_name,
                network_secret: row.network_secret,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at)
                    .map(|d| d.with_timezone(&chrono::Local))
                    .unwrap_or_else(|_| chrono::Local::now()),
                last_connected_at: row.last_connected_at
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
                    .map(|d| d.with_timezone(&chrono::Local)),
                is_auto_connect: row.is_auto_connect,
                status,
                virtual_ip,
                member_count,
                config_json: row.config_json,
            });
        }
        *self.spaces.write().await = spaces.clone();
        Ok(spaces)
    }

    /// 生成分享链接（v2 加密，携带为接收方设置的 IP 与分享者有效配置）
    pub async fn generate_share_link(&self, space_id: &Uuid, ip: Option<String>) -> Result<String, String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        let network_name = space.network_name.clone();
        let network_secret = space.network_secret.clone();
        drop(spaces);

        let effective = self.get_effective_config(space_id).await?;
        let virtual_ip = ip
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let dhcp = Some(virtual_ip.is_none());   // 先借用计算结果
        let info = ShareInfo {
            network_name,
            network_secret,
            host_hint: None,
            virtual_ip,
            dhcp,
            peer_urls: effective.peer_urls.clone(),
            listener_urls: effective.listener_urls.clone(),
        };
        crate::log_info!(format!("生成分享链接: {} (v2 加密)", info.network_name), &space_id.to_string());
        crate::space::share::encrypt_share_payload(&info)
    }

    /// 等待 daemon 就绪（ping 轮询，最多 10s）
    async fn wait_daemon_ready(&self) -> bool {
        for i in 0..50 {
            if self.ipc_client.ping().await {
                return true;
            }
            if i % 10 == 0 {
                crate::log_debug!(format!("connect: 等待 daemon 就绪 ({}/50)...", i + 1));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        false
    }

    /// 连接空间（通过 IPC 通知 daemon）
    pub async fn connect(&self, space_id: &Uuid) -> Result<(), String> {
        crate::log_info!(format!("connect: 开始连接空间, space_id={}", space_id), &space_id.to_string());

        if !self.ipc_client.ping().await {
            crate::log_info!("connect: daemon 未就绪，等待...", &space_id.to_string());
            self.wait_daemon_ready().await;
        }
        crate::log_debug!(format!("connect: 查询当前运行中的空间"), &space_id.to_string());
        let running_spaces: Vec<String> = match self.ipc_client.list_spaces().await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { data }) => {
                data.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        crate::log_info!(format!("connect: 当前运行中空间: {:?}", running_spaces), &space_id.to_string());
        for running_id in &running_spaces {
            if running_id != &space_id.to_string() {
                crate::log_info!(format!("connect: 断开其他空间: {}", running_id), &space_id.to_string());
                let _ = self.ipc_client.disconnect_space(running_id).await;
            }
        }

        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;

        let existing_config = self.db.get_space_config(&space_id.to_string()).ok().flatten();
        if let Some(ref cfg) = existing_config {
            crate::log_info!("connect: 从 DB 加载历史配置", &space_id.to_string());
        } else {
            crate::log_info!("connect: 无历史配置，使用默认配置", &space_id.to_string());
        }

        crate::log_debug!(format!("connect: 调用 get_effective_config"), &space_id.to_string());
        let cfg = self.get_effective_config(space_id).await?;
        crate::log_info!(format!("connect: 有效配置生成完成, network_name={}, dhcp={}", cfg.network_name, cfg.dhcp), &space_id.to_string());

        let config_value = serde_json::to_value(&cfg).map_err(|e| format!("序列化配置失败: {}", e))?;
        crate::log_info!(format!("connect: 发送 IPC ConnectSpace 请求, config_keys={:?}", config_value.as_object().map(|o| o.keys().collect::<Vec<_>>())), &space_id.to_string());
        match self.ipc_client.connect_space(&space_id.to_string(), config_value).await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { .. }) => {
                crate::log_info!(format!("连接空间 IPC 成功: {}", space.name), &space_id.to_string());
                crate::log_debug!("connect: 清理旧聊天服务器", &space_id.to_string());
                if let Some(old) = self.chat_servers.write().await.remove(space_id) {
                    old.stop().await;
                    crate::log_info!(format!("connect: 旧聊天服务器已停止"), &space_id.to_string());
                }
                crate::log_debug!("connect: 清理旧文件服务器", &space_id.to_string());
                if let Some(old) = self.file_servers.write().await.remove(space_id) {
                    old.stop().await;
                    crate::log_info!(format!("connect: 旧文件服务器已停止"), &space_id.to_string());
                }
                crate::log_debug!("connect: 清理旧语音服务器", &space_id.to_string());
                if let Some(mut old) = self.voice_servers.write().await.remove(space_id) {
                    old.shutdown();
                    crate::log_info!(format!("connect: 旧语音服务器已停止"), &space_id.to_string());
                }
                crate::log_debug!("connect: 清理旧屏幕共享服务器", &space_id.to_string());
                if let Some(mut old) = self.screen_servers.write().await.remove(space_id) {
                    old.shutdown();
                    crate::log_info!(format!("connect: 旧屏幕共享服务器已停止"), &space_id.to_string());
                }
                crate::log_debug!(format!("connect: 启动聊天服务器"), &space_id.to_string());
                self.start_chat_server(*space_id).await?;
                crate::log_debug!(format!("connect: 启动文件服务器"), &space_id.to_string());
                self.start_file_server(*space_id).await?;
                crate::log_debug!(format!("connect: 启动语音服务器"), &space_id.to_string());
                self.start_voice_server(*space_id).await?;
                crate::log_debug!(format!("connect: 启动屏幕共享服务器"), &space_id.to_string());
                self.start_screen_share_server(*space_id).await?;

                let cancel_token = CancellationToken::new();
                self.cancel_tokens.write().await.insert(*space_id, cancel_token.clone());

                let space_id_child = *space_id;
                let manager = self.clone();
                let handle = tokio::spawn(async move {
                    if let Err(e) = manager.discover_and_connect_peers(&space_id_child, cancel_token).await {
                        crate::log_error!(format!("discover_and_connect_peers 失败: {}", e), &space_id_child.to_string());
                    }
                });
                self.connect_handles.write().await.insert(*space_id, handle);
                return Ok(());
            }
            Ok(crate::daemon::ipc::IpcResponse::Error { message }) => {
                crate::log_error!(format!("connect: IPC 返回错误: {}", message), &space_id.to_string());
                return Err(message);
            }
            Err(e) => {
                crate::log_error!(format!("connect: IPC 调用失败: {}", e), &space_id.to_string());
                return Err(e);
            }
        }
    }

    /// 退出应用前断开所有运行中的空间
    pub async fn shutdown_all(&self) {
        let spaces = self.spaces.read().await;
        for space in spaces.iter() {
            if space.status == SpaceStatus::Connected {
                crate::log_info!(format!("[退出] 断开空间: {}", space.id), &space.id.to_string());
                let _ = self.disconnect(&space.id).await;
            }
        }
    }

    /// 断开空间（取消背景任务、停止所有服务、通过 IPC 通知 daemon）
    pub async fn disconnect(&self, space_id: &Uuid) -> Result<(), String> {
        crate::log_info!(format!("断开空间: {}", space_id), &space_id.to_string());

        // 1. 取消 background discover_and_connect_peers 任务
        if let Some(token) = self.cancel_tokens.write().await.remove(space_id) {
            token.cancel();
            crate::log_debug!(format!("disconnect: 已发送取消令牌"), &space_id.to_string());
        }

        // 2. 等待 background 任务结束（如果有句柄）
        if let Some(handle) = self.connect_handles.write().await.remove(space_id) {
            let _ = handle.await;
            crate::log_debug!(format!("disconnect: background 任务已结束"), &space_id.to_string());
        }

        // 3. 停止所有本地服务
        if let Some(old) = self.chat_servers.write().await.remove(space_id) {
            old.stop().await;
        }
        if let Some(mut server) = self.voice_servers.write().await.remove(space_id) {
            server.shutdown();
        }
        if let Some(mut server) = self.screen_servers.write().await.remove(space_id) {
            server.shutdown();
        }
        if let Some(old) = self.file_servers.write().await.remove(space_id) {
            old.stop().await;
        }

        // 4. 清理 chat 客户端
        self.chat_clients.write().await.remove(space_id);

        // 5. 通过 IPC 通知 daemon 断开网络
        match self.ipc_client.disconnect_space(&space_id.to_string()).await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { .. }) => {
                crate::log_info!(format!("断开空间完成: {}", space_id), &space_id.to_string());
                Ok(())
            }
            Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
            Err(e) => Err(e),
        }
    }

    /// 获取空间运行时状态（通过 RPC 查询）
    pub async fn get_space_status(&self, space_id: &str) -> Result<Option<serde_json::Value>, String> {
        match self.ipc_client.get_space_status(space_id).await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { data }) => Ok(data),
            Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
            Err(e) => Err(e),
        }
    }

    /// 运行时修改空间配置
    pub async fn patch_config(&self, space_id: &str, patch: serde_json::Value) -> Result<(), String> {
        // 先保存到数据库
        if let Ok(Some(existing)) = self.db.get_space_config(space_id) {
            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&existing) {
                // 合并 patch
                if let (Some(obj), Some(patch_obj)) = (config.as_object_mut(), patch.as_object()) {
                    for (key, value) in patch_obj {
                        obj.insert(key.clone(), value.clone());
                    }
                }
                let _ = self.db.update_space_config(space_id, &config.to_string());
            }
        }

        // 通过 IPC 通知 daemon 应用配置
        match self.ipc_client.patch_config(space_id, patch).await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { .. }) => Ok(()),
            Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
            Err(e) => Err(e),
        }
    }

    /// 启动聊天服务器
    async fn start_chat_server(&self, space_id: Uuid) -> Result<(), String> {
        let chat_port = 18000 + (space_id.as_u128() % 1000) as u16;

        let mut server = ChatServer::new();
        server.start(chat_port).await.map_err(|e| format!("启动聊天服务器失败: {}", e))?;

        self.chat_servers.write().await.insert(space_id, server);
        crate::log_info!(format!("聊天服务器已启动: space_id={}, port={}", space_id, chat_port), &space_id.to_string());
        Ok(())
    }

    /// 启动语音服务器
    async fn start_voice_server(&self, space_id: Uuid) -> Result<(), String> {
        let voice_port = 18100 + (space_id.as_u128() % 1000) as u16;

        let mut server = VoiceServer::new(voice_port);
        server.start().await.map_err(|e| format!("启动语音服务器失败: {}", e))?;

        self.voice_servers.write().await.insert(space_id, server);
        crate::log_info!(format!("语音服务器已启动: space_id={}, port={}", space_id, voice_port), &space_id.to_string());
        Ok(())
    }

    /// 启动屏幕共享服务器
    async fn start_screen_share_server(&self, space_id: Uuid) -> Result<(), String> {
        let screen_port = 18200 + (space_id.as_u128() % 1000) as u16;

        let mut server = ScreenShareSignalServer::new(screen_port);
        server.start().await.map_err(|e| format!("启动屏幕共享服务器失败: {}", e))?;

        self.screen_servers.write().await.insert(space_id, server);
        crate::log_info!(format!("屏幕共享服务器已启动: space_id={}, port={}", space_id, screen_port), &space_id.to_string());
        Ok(())
    }

    /// 启动文件服务器
    async fn start_file_server(&self, space_id: Uuid) -> Result<(), String> {
        let base = crate::config::get_u16(crate::config::KEY_FILE_SERVER_PORT_BASE, crate::config::DEFAULT_FILE_SERVER_PORT_BASE);
        let file_port = base + (space_id.as_u128() % 1000) as u16;
        let storage_dir = self.storage_dir.read().await.join(space_id.to_string());

        let mut server = FileServer::new(space_id, storage_dir);
        server.start(file_port).await.map_err(|e| format!("启动文件服务器失败: {}", e))?;

        self.file_servers.write().await.insert(space_id, server);
        crate::log_info!(format!("文件服务器已启动: space_id={}, port={}", space_id, file_port), &space_id.to_string());
        Ok(())
    }

    /// 发现并连接到 peers（支持取消令牌）
    async fn discover_and_connect_peers(&self, space_id: &Uuid, cancel_token: CancellationToken) -> Result<(), String> {
        const RETRY_DELAYS: &[u64] = &[1, 2, 3, 5, 7, 10, 10];
        let max_retries = RETRY_DELAYS.len();

        let virtual_ip = {
            let mut retries = 0;
            loop {
                if cancel_token.is_cancelled() {
                    return Err("连接已取消".to_string());
                }

                let status = match self.get_space_status(&space_id.to_string()).await {
                    Ok(s) => s,
                    Err(e) => {
                        retries += 1;
                        if retries >= max_retries {
                            crate::log_error!(format!(
                                "discover_and_connect_peers: get_space_status retry exhausted after {} attempts, last error: {}",
                                max_retries, e
                            ), &space_id.to_string());
                            return Err(format!("查询空间状态失败(已重试{}次): {}", max_retries, e));
                        }
                        let delay = RETRY_DELAYS[retries - 1];
                        crate::log_warn!(format!(
                            "discover_and_connect_peers: get_space_status 失败(第{}次), 等待 {}s 重试: {}",
                            retries, delay, e
                        ), &space_id.to_string());
                        tokio::select! {
                            _ = cancel_token.cancelled() => return Err("连接已取消".to_string()),
                            _ = tokio::time::sleep(std::time::Duration::from_secs(delay)) => {}
                        }
                        continue;
                    }
                };
                if let Some(status_data) = status {
                    if let Some(ip) = status_data.get("virtual_ip").and_then(|v| v.as_str()) {
                        if !ip.is_empty() {
                            break ip.to_string();
                        }
                    }
                }
                retries += 1;
                if retries >= max_retries {
                    return Err("未获取到虚拟 IP".to_string());
                }
                let delay = RETRY_DELAYS[retries - 1];
                crate::log_info!(format!(
                    "discover_and_connect_peers: 虚拟 IP 尚未就绪 (第{}次), 等待 {}s 重试",
                    retries, delay
                ), &space_id.to_string());
                tokio::select! {
                    _ = cancel_token.cancelled() => return Err("连接已取消".to_string()),
                    _ = tokio::time::sleep(std::time::Duration::from_secs(delay)) => {}
                }
            }
        };

        let my_chat_port = 18000 + (space_id.as_u128() % 1000) as u16;

        // 获取 peer 列表（通过 RPC）
        let peer_list = self.get_peers(space_id).await?;
        let mut peers_map = HashMap::new();

        for peer in peer_list {
            // 过滤本机，避免广播/定向信令回环
            if peer.is_local {
                continue;
            }
            if let Some(peer_ip) = peer.virtual_ip {
                // peer 的聊天端口也是基于 space_id 计算
                let peer_chat_port = 18000 + (space_id.as_u128() % 1000) as u16;
                // 以虚拟 IP 为 key，前端可直接用成员 virtual_ip 定向发送
                peers_map.insert(peer_ip.clone(), (peer_ip, peer_chat_port));
            }
        }

        // 更新 ChatClient
        let mut clients = self.chat_clients.write().await;
        let client = clients.entry(*space_id).or_insert_with(ChatClient::new);
        let peer_count = peers_map.len();
        client.update_peers(peers_map);

        crate::log_info!(format!("已连接到 {} 个 peers", peer_count), &space_id.to_string());
        Ok(())
    }

    /// 广播消息到所有 peers
    pub async fn broadcast_message(&self, msg: &crate::chat::message::ChatMessage) -> Vec<(String, String)> {
        let clients = self.chat_clients.read().await;
        if let Some(client) = clients.get(&msg.space_id) {
            client.broadcast(msg).await
        } else {
            Vec::new()
        }
    }

    /// 获取当前已连接的可达 peer 数量
    pub async fn chat_peer_count(&self, space_id: &Uuid) -> usize {
        let clients = self.chat_clients.read().await;
        clients.get(space_id).map(|c| c.peer_count()).unwrap_or(0)
    }

    /// 定向发送信令到指定成员
    pub async fn send_signal_to(&self, space_id: &Uuid, target: &str, msg: &crate::chat::message::ChatMessage) -> Result<(), String> {
        let clients = self.chat_clients.read().await;
        if let Some(client) = clients.get(space_id) {
            client.send_to(target, msg).await
        } else {
            Err("空间未连接，ChatClient 不存在".to_string())
        }
    }

    /// 获取 peer 列表（通过 RPC 查询）
    pub async fn get_peers(&self, space_id: &Uuid) -> Result<Vec<crate::easytier::launcher::PeerInfo>, String> {
        match self.ipc_client.list_peers(&space_id.to_string()).await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { data }) => {
                if let Some(v) = data {
                    serde_json::from_value(v).map_err(|e| format!("解析 peer 列表失败: {}", e))
                } else {
                    Ok(Vec::new())
                }
            }
            Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
            Err(e) => Err(e),
        }
    }

    /// 获取有效配置（合并组配置和本地配置）
    pub async fn get_effective_config(&self, space_id: &Uuid) -> Result<NetworkConfig, String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;

        // 从 DB 加载组配置 (config_json) 作为基础配置
        let base_config = match self.db.get_space_config(&space_id.to_string()) {
            Ok(Some(json)) => {
                crate::log_info!(format!("get_effective_config: 加载 config_json: {}", json), &space_id.to_string());
                match NetworkConfig::from_config_json(&json) {
                    Ok(mut cfg) => {
                        crate::log_info!(format!("get_effective_config: config_json 解析成功: virtual_ipv4={}, network_name={}, instance_id={}", cfg.virtual_ipv4, cfg.network_name, cfg.instance_id), &space_id.to_string());
                        // 从 config_json 解析成功，补充 identity 字段（防止 config_json 中缺失）
                        if cfg.network_name.is_empty() {
                            cfg.network_name = space.network_name.clone();
                        }
                        if cfg.network_secret.is_empty() {
                            cfg.network_secret = space.network_secret.clone();
                        }
                        cfg
                    }
                    Err(e) => {
                        crate::log_warn!(format!("get_effective_config: config_json 解析失败，使用空间基础配置: {}", e), &space_id.to_string());
                        NetworkConfig {
                            network_name: space.network_name.clone(),
                            network_secret: space.network_secret.clone(),
                            dhcp: true,
                            ..Default::default()
                        }
                    }
                }
            }
            Ok(None) => {
                NetworkConfig {
                    network_name: space.network_name.clone(),
                    network_secret: space.network_secret.clone(),
                    dhcp: true,
                    ..Default::default()
                }
            }
            Err(e) => return Err(format!("读取空间配置失败: {}", e)),
        };

        let mut config = base_config;

        Ok(config)
    }

    /// 获取用于文件传输的 peer (IP, port) 列表
    pub async fn get_peers_for_file_transfer(&self, space_id: &Uuid) -> Result<Vec<(String, u16)>, String> {
        let peers = self.get_peers(space_id).await?;
        let base = crate::config::get_u16(crate::config::KEY_FILE_SERVER_PORT_BASE, crate::config::DEFAULT_FILE_SERVER_PORT_BASE);
        let file_port = base + (space_id.as_u128() % 1000) as u16;

        let mut results = Vec::new();
        for peer in peers {
            if let Some(virtual_ip) = peer.virtual_ip {
                results.push((virtual_ip, file_port));
            }
        }
        Ok(results)
    }

    /// 获取文件列表
    pub async fn list_space_files(&self, space_id: &str, limit: Option<u32>) -> Result<Vec<crate::db::models::FileRow>, String> {
        self.db.list_files(space_id, limit)
    }

    /// 校验是否为空间创建者
    pub async fn check_owner(&self, space_id: &str) -> Result<(), String> {
        let caller_id = self.db.get_user_id()?.unwrap_or_default();
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| s.id.to_string() == *space_id)
            .ok_or_else(|| "空间不存在".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id.as_str()) {
            return Err("无权限：仅空间创建者可执行此操作".to_string());
        }
        Ok(())
    }

    /// 获取空间成员列表
    pub async fn list_members(&self, space_id: &Uuid) -> Result<Vec<Member>, String> {
        let rows = self.db.list_members(&space_id.to_string())?;
        let members = rows.iter().map(|r| {
            Member {
                id: r.id.parse().unwrap_or_default(),
                space_id: r.space_id.parse().unwrap_or_default(),
                nickname: r.nickname.clone(),
                virtual_ip: r.virtual_ip.clone(),
                is_online: r.is_online,
                is_owner: r.is_owner,
                joined_at: chrono::DateTime::parse_from_rfc3339(&r.joined_at)
                    .map(|d| d.with_timezone(&chrono::Local))
                    .unwrap_or_else(|_| chrono::Local::now()),
                last_seen_at: r.last_seen_at.as_ref().and_then(|t| {
                    chrono::DateTime::parse_from_rfc3339(t).ok().map(|d| d.with_timezone(&chrono::Local))
                }),
            }
        }).collect();
        Ok(members)
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
impl SpaceManager {
    pub fn new(db: Arc<Database>, easytier: Arc<crate::easytier::EasyTierManager>) -> Self {
        Self { db, easytier, spaces: Arc::new(RwLock::new(Vec::new())) }
    }

    /// 创建空间（Mobile: 库方式）
    pub async fn create(&self, name: String, network_secret: String, description: Option<String>) -> Result<Space, String> {
        let space_id = Uuid::new_v4();
        let owner_uuid = self.db.get_user_id()?.unwrap_or_else(|| "local-user".to_string());
        let network_name = name.clone();

        let space = Space {
            id: space_id,
            name,
            description,
            owner_id: Some(owner_uuid.clone()),
            network_name: network_name.clone(),
            network_secret: network_secret.clone(),
            created_at: chrono::Local::now(),
            last_connected_at: None,
            is_auto_connect: false,
            status: SpaceStatus::Disconnected,
            virtual_ip: None,
            member_count: 1,
            config_json: None,
        };

        let row = SpaceRow {
            id: space.id.to_string(),
            name: space.name.clone(),
            owner_id: space.owner_id.clone(),
            network_name: space.network_name.clone(),
            network_secret: space.network_secret.clone(),
            description: space.description.clone(),
            created_at: space.created_at.to_rfc3339(),
            last_connected_at: None,
            is_auto_connect: false,
                config_json: None,
        };
        self.db.insert_space(&row)?;
        self.db.add_member(&space_id.to_string(), &owner_uuid, &space.name, true)?;
        self.spaces.write().await.push(space.clone());
        crate::log_info!(format!("创建空间: {} (id={}, owner={})", space.name, space.id, owner_uuid), &space.id.to_string());
        Ok(space)
    }

    /// 加入空间
    /// config 为前端传入的 easytier 配置 json 反序列化结果，缺省字段已由 serde(default) 补全，
    /// 完整配置序列化后写入 config_json 落库
    pub async fn join(&self, config: NetworkConfig) -> Result<Space, String> {
        let network_name = config.network_name.clone();
        let network_secret = config.network_secret.clone();
        let space = Space {
            id: Uuid::new_v4(),
            name: network_name.clone(),
            description: None,
            owner_id: None,
            network_name: network_name.clone(),
            network_secret: network_secret.clone(),
            created_at: chrono::Local::now(),
            last_connected_at: None,
            is_auto_connect: false,
            status: SpaceStatus::Disconnected,
            virtual_ip: None,
            member_count: 1,
            config_json: None,
        };

        let row = SpaceRow {
            id: space.id.to_string(),
            name: space.name.clone(),
            owner_id: None,
            network_name: space.network_name.clone(),
            network_secret: space.network_secret.clone(),
            description: None,
            created_at: space.created_at.to_rfc3339(),
            last_connected_at: None,
            is_auto_connect: false,
                config_json: None,
        };
        self.db.insert_space(&row)?;

        // 完整配置 json 落库（默认值已由后端 serde(default) 补全）
        let config_json = serde_json::to_string(&config)
            .map_err(|e| format!("序列化配置失败: {}", e))?;
        self.db.update_space_config(&space.id.to_string(), &config_json)?;
        crate::log_info!(
            format!("加入空间: 配置已写入 config json (dhcp={}, ip={}, peers={}, listeners={})",
                config.dhcp,
                config.virtual_ipv4,
                config.peer_urls.len(),
                config.listener_urls.len()),
            &space.id.to_string()
        );

        self.spaces.write().await.push(space.clone());
        crate::log_info!(format!("加入空间: {}", space.name), &space.id.to_string());
        Ok(space)
    }

    /// 离开空间
    pub async fn leave(&self, space_id: &Uuid) -> Result<(), String> {
        if self.easytier.is_running(space_id) {
            self.easytier.stop_network(space_id).await?;
        }
        crate::log_info!(format!("离开空间: {}", space_id), &space_id.to_string());
        Ok(())
    }

    /// 删除空间
    pub async fn delete(&self, space_id: &Uuid) -> Result<(), String> {
        let caller_id = self.db.get_user_id()?.unwrap_or_default();
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id.as_str()) {
            return Err("只有空间所有者才能删除空间".to_string());
        }
        drop(spaces);

        self.leave(space_id).await?;
        self.db.delete_space(&space_id.to_string())?;
        self.spaces.write().await.retain(|s| s.id != *space_id);
        crate::log_info!(format!("空间已删除: {}", space_id), &space_id.to_string());
        Ok(())
    }

    /// 获取空间列表
    pub async fn list(&self) -> Result<Vec<Space>, String> {
        let rows = self.db.list_spaces()?;
        let mut spaces = Vec::new();
        for row in rows {
            let id: Uuid = row.id.parse().unwrap_or_default();
            let is_running = self.easytier.is_running(&id);
            let status = if is_running { SpaceStatus::Connected } else { SpaceStatus::Disconnected };
            let member_count = if is_running { self.easytier.get_connected_peers(&id).unwrap_or(0) + 1 } else { 0 };
            let virtual_ip = if is_running { self.easytier.get_virtual_ip(&id) } else { None };

            spaces.push(Space {
                id,
                name: row.name,
                description: row.description,
                owner_id: row.owner_id,
                network_name: row.network_name,
                network_secret: row.network_secret,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at)
                    .map(|d| d.with_timezone(&chrono::Local))
                    .unwrap_or_else(|_| chrono::Local::now()),
                last_connected_at: row.last_connected_at
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
                    .map(|d| d.with_timezone(&chrono::Local)),
                is_auto_connect: row.is_auto_connect,
                status,
                virtual_ip,
                member_count,
                config_json: row.config_json,
            });
        }
        *self.spaces.write().await = spaces.clone();
        Ok(spaces)
    }

    /// 生成分享链接（Mobile: 基于空间基础信息加密）
    pub async fn generate_share_link(&self, space_id: &Uuid, ip: Option<String>) -> Result<String, String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        let virtual_ip = ip
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let info = ShareInfo {
            network_name: space.network_name.clone(),
            network_secret: space.network_secret.clone(),
            host_hint: None,
            virtual_ip,
            dhcp: Some(virtual_ip.is_none()),
            peer_urls: Vec::new(),
            listener_urls: Vec::new(),
        };
        crate::log_info!(format!("生成分享链接: {} (v2 加密)", info.network_name), &space_id.to_string());
        crate::space::share::encrypt_share_payload(&info)
    }

    /// 连接空间（Mobile: 直接调用库）
    pub async fn connect(&self, space_id: &Uuid) -> Result<(), String> {
        let running = self.easytier.list_running();
        for running_id in &running {
            let _ = self.easytier.stop_network(running_id).await;
        }

        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;

        let existing_config = self.db.get_space_config(&space_id.to_string()).ok().flatten();

        let cfg = crate::easytier::config::NetworkConfig {
            network_name: space.network_name.clone(),
            network_secret: space.network_secret.clone(),
            ..Default::default()
        };

        self.easytier.start_network(cfg, *space_id, existing_config).await?;
        crate::log_info!(format!("连接空间: {}", space.name), &space_id.to_string());
        Ok(())
    }

    /// 断开空间
    pub async fn disconnect(&self, space_id: &Uuid) -> Result<(), String> {
        crate::log_info!(format!("断开空间: {}", space_id), &space_id.to_string());
        self.easytier.stop_network(space_id).await?;
        Ok(())
    }

    /// 校验是否为空间创建者
    pub async fn check_owner(&self, space_id: &str) -> Result<(), String> {
        let caller_id = self.db.get_user_id()?.unwrap_or_default();
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| s.id.to_string() == *space_id)
            .ok_or_else(|| "空间不存在".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id.as_str()) {
            return Err("无权限：仅空间创建者可执行此操作".to_string());
        }
        Ok(())
    }

    /// 获取空间成员列表
    pub async fn list_members(&self, space_id: &Uuid) -> Result<Vec<Member>, String> {
        let rows = self.db.list_members(&space_id.to_string())?;

        // 构建在线虚拟 IP 集合：本机 IP + 已连接 peer 的 IP
        let mut online_ips: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Some(local_ip) = self.easytier.get_virtual_ip(space_id) {
            online_ips.insert(local_ip);
        }
        if let Ok(peers) = self.easytier.get_peers(space_id).await {
            for p in peers {
                if p.connected {
                    if let Some(ip) = p.virtual_ip {
                        online_ips.insert(ip);
                    }
                }
            }
        }

        let members = rows.iter().map(|r| {
            let is_online = match &r.virtual_ip {
                Some(ip) => online_ips.contains(ip),
                None => false,
            };
            Member {
                id: r.id.parse().unwrap_or_default(),
                space_id: r.space_id.parse().unwrap_or_default(),
                nickname: r.nickname.clone(),
                virtual_ip: r.virtual_ip.clone(),
                is_online,
                is_owner: r.is_owner,
                joined_at: chrono::DateTime::parse_from_rfc3339(&r.joined_at)
                    .map(|d| d.with_timezone(&chrono::Local))
                    .unwrap_or_else(|_| chrono::Local::now()),
                last_seen_at: r.last_seen_at.as_ref().and_then(|t| {
                    chrono::DateTime::parse_from_rfc3339(t).ok().map(|d| d.with_timezone(&chrono::Local))
                }),
            }
        }).collect();
        Ok(members)
    }
}
