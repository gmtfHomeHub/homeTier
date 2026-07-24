use std::sync::Arc;
use uuid::Uuid;
use tokio::sync::RwLock;
use crate::db::Database;
use crate::types::{Space, SpaceStatus, Member, ShareInfo};
use crate::db::models::SpaceRow;

/// 空间管理器
pub struct SpaceManager {
    db: Arc<Database>,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    easytier: Arc<crate::easytier::EasyTierManager>,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ipc_client: Arc<crate::daemon::client::IpcClient>,
    spaces: Arc<RwLock<Vec<Space>>>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl SpaceManager {
    pub fn new(
        db: Arc<Database>,
        _easytier: Arc<crate::easytier::EasyTierManager>,
        ipc_client: Arc<crate::daemon::client::IpcClient>,
    ) -> Self {
        Self { db, ipc_client, spaces: Arc::new(RwLock::new(Vec::new())) }
    }

    /// 创建空间（创建者自动成为 owner）
    pub async fn create(&self, name: String, network_secret: String, owner_id: String, description: Option<String>) -> Result<Space, String> {
        let space_id = Uuid::new_v4();
        let owner_uuid = uuid::Uuid::parse_str(&owner_id).unwrap_or_else(|_| space_id);
        let network_name = name.clone();

        let space = Space {
            id: space_id,
            name,
            description,
            owner_id: Some(owner_uuid.to_string()),
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
            local_config_json: None,
        };
        self.db.insert_space(&row)?;
        self.db.add_member(&space_id.to_string(), &owner_uuid.to_string(), &space.name, true)?;
        self.spaces.write().await.push(space.clone());

        crate::log_info!(format!("创建空间: {} (id={}, owner={})", space.name, space.id, owner_uuid));
        Ok(space)
    }

    /// 加入空间
    pub async fn join(&self, network_name: String, network_secret: String) -> Result<Space, String> {
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
            local_config_json: None,
        };
        self.db.insert_space(&row)?;
        self.spaces.write().await.push(space.clone());
        crate::log_info!(format!("加入空间: {}", space.name));
        Ok(space)
    }

    /// 离开空间
    pub async fn leave(&self, space_id: &Uuid) -> Result<(), String> {
        self.disconnect(space_id).await?;
        crate::log_info!(format!("离开空间: {}", space_id), &space_id.to_string());
        Ok(())
    }

    /// 删除空间
    pub async fn delete(&self, space_id: &Uuid, caller_id: &str) -> Result<(), String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id) {
            return Err("只有空间所有者才能删除空间".to_string());
        }
        drop(spaces);

        self.leave(space_id).await?;
        self.db.delete_space(&space_id.to_string())?;
        self.spaces.write().await.retain(|s| s.id != *space_id);
        crate::log_info!(format!("空间已删除: {}", space_id), &space_id.to_string());
        Ok(())
    }

    /// 移除空间成员
    pub async fn remove_member(&self, space_id: &Uuid, target_member_id: &str, caller_id: &str) -> Result<(), String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id) {
            return Err("只有空间所有者才能移除成员".to_string());
        }
        drop(spaces);

        if caller_id == target_member_id {
            return Err("不能移除自己".to_string());
        }

        self.db.remove_member(&space_id.to_string(), target_member_id)?;
        crate::log_info!(format!("成员已移除: member={} from space={}", target_member_id, space_id), &space_id.to_string());
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

    /// 生成分享链接
    pub fn generate_share_link(&self, space: &Space) -> String {
        format!("homeTier://join?name={}&secret={}", space.network_name, space.network_secret)
    }

    /// 解析分享链接
    pub fn parse_share_link(link: &str) -> Result<ShareInfo, String> {
        let url = url::Url::parse(link).map_err(|_| "Invalid share link".to_string())?;
        if url.scheme() != "homeTier" || url.host_str() != Some("join") {
            return Err("Invalid share link format".to_string());
        }
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let network_name = pairs.get("name")
            .ok_or_else(|| "Missing network name".to_string())?
            .to_string();
        let network_secret = pairs.get("secret")
            .ok_or_else(|| "Missing network secret".to_string())?
            .to_string();
        Ok(ShareInfo { network_name, network_secret, host_hint: None })
    }

    /// 连接空间（通过 IPC 通知 daemon）
    pub async fn connect(&self, space_id: &Uuid) -> Result<(), String> {
        // 断开所有其他已连接的空间
        let running_spaces: Vec<String> = match self.ipc_client.list_spaces().await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { data }) => {
                data.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        for running_id in &running_spaces {
            if running_id != &space_id.to_string() {
                let _ = self.ipc_client.disconnect_space(running_id).await;
            }
        }

        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;

        // 加载保存的配置
        let existing_config = self.db.get_space_config(&space_id.to_string()).ok().flatten();
        if let Some(ref cfg) = existing_config {
            crate::log_info!("connect: 从 DB 加载历史配置", &space_id.to_string());
        }

        // 构建 NetworkConfig（优先使用保存的配置）
        let cfg = if let Some(ref config_str) = existing_config {
            // 尝试从保存的 JSON 解析 NetworkConfig
            serde_json::from_str::<crate::easytier::config::NetworkConfig>(config_str)
                .unwrap_or_else(|_| crate::easytier::config::NetworkConfig {
                    network_name: space.network_name.clone(),
                    network_secret: space.network_secret.clone(),
                    ..Default::default()
                })
        } else {
            crate::easytier::config::NetworkConfig {
                network_name: space.network_name.clone(),
                network_secret: space.network_secret.clone(),
                ..Default::default()
            }
        };

        // 通过 IPC 连接
        let config_value = serde_json::to_value(&cfg).map_err(|e| format!("序列化配置失败: {}", e))?;
        match self.ipc_client.connect_space(&space_id.to_string(), config_value).await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { .. }) => {
                crate::log_info!(format!("连接空间: {}", space.name), &space_id.to_string());
                Ok(())
            }
            Ok(crate::daemon::ipc::IpcResponse::Error { message }) => Err(message),
            Err(e) => Err(e),
        }
    }

    /// 断开空间
    pub async fn disconnect(&self, space_id: &Uuid) -> Result<(), String> {
        crate::log_info!(format!("断开空间: {}", space_id), &space_id.to_string());
        match self.ipc_client.disconnect_space(&space_id.to_string()).await {
            Ok(crate::daemon::ipc::IpcResponse::Ok { .. }) => Ok(()),
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

    /// 校验是否为空间创建者
    pub async fn check_owner(&self, space_id: &str, caller_id: &str) -> Result<(), String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| s.id.to_string() == *space_id)
            .ok_or_else(|| "空间不存在".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id) {
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
    pub async fn create(&self, name: String, network_secret: String, owner_id: String, description: Option<String>) -> Result<Space, String> {
        let space_id = Uuid::new_v4();
        let owner_uuid = uuid::Uuid::parse_str(&owner_id).unwrap_or_else(|_| space_id);
        let network_name = name.clone();

        let space = Space {
            id: space_id,
            name,
            description,
            owner_id: Some(owner_uuid.to_string()),
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
            local_config_json: None,
        };
        self.db.insert_space(&row)?;
        self.db.add_member(&space_id.to_string(), &owner_uuid.to_string(), &space.name, true)?;
        self.spaces.write().await.push(space.clone());
        crate::log_info!(format!("创建空间: {} (id={}, owner={})", space.name, space.id, owner_uuid));
        Ok(space)
    }

    /// 加入空间
    pub async fn join(&self, network_name: String, network_secret: String) -> Result<Space, String> {
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
            local_config_json: None,
        };
        self.db.insert_space(&row)?;
        self.spaces.write().await.push(space.clone());
        crate::log_info!(format!("加入空间: {}", space.name));
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
    pub async fn delete(&self, space_id: &Uuid, caller_id: &str) -> Result<(), String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id) {
            return Err("只有空间所有者才能删除空间".to_string());
        }
        drop(spaces);

        self.leave(space_id).await?;
        self.db.delete_space(&space_id.to_string())?;
        self.spaces.write().await.retain(|s| s.id != *space_id);
        crate::log_info!(format!("空间已删除: {}", space_id), &space_id.to_string());
        Ok(())
    }

    /// 移除空间成员
    pub async fn remove_member(&self, space_id: &Uuid, target_member_id: &str, caller_id: &str) -> Result<(), String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id) {
            return Err("只有空间所有者才能移除成员".to_string());
        }
        drop(spaces);

        if caller_id == target_member_id {
            return Err("不能移除自己".to_string());
        }

        self.db.remove_member(&space_id.to_string(), target_member_id)?;
        crate::log_info!(format!("成员已移除: member={} from space={}", target_member_id, space_id), &space_id.to_string());
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

    /// 生成分享链接
    pub fn generate_share_link(&self, space: &Space) -> String {
        format!("homeTier://join?name={}&secret={}", space.network_name, space.network_secret)
    }

    /// 解析分享链接
    pub fn parse_share_link(link: &str) -> Result<ShareInfo, String> {
        let url = url::Url::parse(link).map_err(|_| "Invalid share link".to_string())?;
        if url.scheme() != "homeTier" || url.host_str() != Some("join") {
            return Err("Invalid share link format".to_string());
        }
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let network_name = pairs.get("name")
            .ok_or_else(|| "Missing network name".to_string())?
            .to_string();
        let network_secret = pairs.get("secret")
            .ok_or_else(|| "Missing network secret".to_string())?
            .to_string();
        Ok(ShareInfo { network_name, network_secret, host_hint: None })
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
    pub async fn check_owner(&self, space_id: &str, caller_id: &str) -> Result<(), String> {
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| s.id.to_string() == *space_id)
            .ok_or_else(|| "空间不存在".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id) {
            return Err("无权限：仅空间创建者可执行此操作".to_string());
        }
        Ok(())
    }

    /// 获取空间成员列表
    pub async fn list_members(&self, space_id: &Uuid) -> Result<Vec<Member>, String> {
        let rows = self.db.list_members(&space_id.to_string())?;
        let members = rows.iter().map(|r| {
            let is_online = self.easytier.get_connected_peers(space_id).unwrap_or(0) > 0;
            Member {
                id: r.id.parse().unwrap_or_default(),
                space_id: r.space_id.parse().unwrap_or_default(),
                nickname: r.nickname.clone(),
                virtual_ip: r.virtual_ip.clone(),
                is_online: r.is_online || is_online,
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
