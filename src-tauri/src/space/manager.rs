use std::sync::Arc;
use uuid::Uuid;
use tokio::sync::RwLock;
use crate::db::Database;
use crate::easytier::EasyTierManager;
use crate::types::{Space, SpaceStatus, Member, ShareInfo};
use crate::db::models::SpaceRow;

/// 空间管理器
pub struct SpaceManager {
    db: Arc<Database>,
    easytier: Arc<EasyTierManager>,
    spaces: Arc<RwLock<Vec<Space>>>,
}

impl SpaceManager {
    pub fn new(db: Arc<Database>, easytier: Arc<EasyTierManager>) -> Self {
        Self { db, easytier, spaces: Arc::new(RwLock::new(Vec::new())) }
    }

    /// 创建空间（创建者自动成为 owner）
    pub async fn create(&self, name: String, network_secret: String, owner_id: String, description: Option<String>) -> Result<Space, String> {
        let space_id = Uuid::new_v4();
        let owner_uuid = uuid::Uuid::parse_str(&owner_id).unwrap_or_else(|_| space_id);
        // 使用空间名称作为网络标识名，保持原始名称
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

        // 持久化到数据库
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

        // 添加创建者为 owner 成员
        self.db.add_member(&space_id.to_string(), &owner_uuid.to_string(), &space.name, true)?;

        // 添加到内存
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

        // 持久化
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

    /// 离开空间（停止网络但不删除记录）
    pub async fn leave(&self, space_id: &Uuid) -> Result<(), String> {
        // 停止 EasyTier 网络实例
        if self.easytier.is_running(space_id) {
            self.easytier.stop_network(space_id).await?;
        }
        crate::log_info!(format!("离开空间: {}", space_id), &space_id.to_string());
        Ok(())
    }

    /// 删除空间（仅 owner 可操作）
    pub async fn delete(&self, space_id: &Uuid, caller_id: &str) -> Result<(), String> {
        // 校验 owner 身份
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

    /// 移除空间成员（仅 owner 可操作）
    pub async fn remove_member(&self, space_id: &Uuid, target_member_id: &str, caller_id: &str) -> Result<(), String> {
        // 校验 caller 是 owner
        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;
        if space.owner_id.as_deref() != Some(caller_id) {
            return Err("只有空间所有者才能移除成员".to_string());
        }
        drop(spaces);

        // 不能移除自己（owner 自己）
        if caller_id == target_member_id {
            return Err("不能移除自己".to_string());
        }

        self.db.remove_member(&space_id.to_string(), target_member_id)?;
        crate::log_info!(format!("成员已移除: member={} from space={}", target_member_id, space_id), &space_id.to_string());
        Ok(())
    }

    /// 获取空间列表
    pub async fn list(&self) -> Result<Vec<Space>, String> {
        // 先从数据库加载
        let rows = self.db.list_spaces()?;
        let mut spaces = Vec::new();
        for row in rows {
            let id: Uuid = row.id.parse().unwrap_or_default();
            let is_running = self.easytier.is_running(&id);
            let status = if is_running {
                SpaceStatus::Connected
            } else {
                SpaceStatus::Disconnected
            };
            let member_count = if is_running {
                self.easytier.get_connected_peers(&id).unwrap_or(0) + 1
            } else {
                0
            };
            let virtual_ip = if is_running {
                self.easytier.get_virtual_ip(&id)
            } else {
                None
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
        format!(
            "homeTier://join?name={}&secret={}",
            space.network_name,
            space.network_secret
        )
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

    /// 连接空间（启动 EasyTier 网络），互斥：同一时间只有一个空间在线
    pub async fn connect(&self, space_id: &Uuid) -> Result<(), String> {
        // 先断开所有其他已连接的空间（互斥），包括当前空间（支持重连）
        let running = self.easytier.list_running();
        for running_id in &running {
            crate::log_info!(format!("connect: 断开已连接的空间 {}", running_id), &running_id.to_string());
            let _ = self.easytier.stop_network(running_id).await;
        }

        let spaces = self.spaces.read().await;
        let space = spaces.iter().find(|s| &s.id == space_id)
            .ok_or_else(|| "Space not found".to_string())?;

        // 从 DB 加载历史配置
        let existing_config = self.db.get_space_config(&space_id.to_string()).ok().flatten();
        if let Some(ref cfg) = existing_config {
            crate::log_info!("connect: 从 DB 加载历史配置", &space_id.to_string());
        }

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