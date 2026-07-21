pub mod config;
pub mod launcher;

use std::sync::Arc;
use dashmap::DashMap;
use uuid::Uuid;
use tokio::sync::RwLock;
use std::path::PathBuf;
use crate::types::NetworkStatus;

/// EasyTier 管理器，管理多个网络实例
pub struct EasyTierManager {
    instances: DashMap<Uuid, launcher::RunningInstance>,
    config_dir: PathBuf,
}

impl EasyTierManager {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            instances: DashMap::new(),
            config_dir,
        }
    }

    /// 启动网络实例
    pub async fn start_network(
        &self,
        cfg: config::NetworkConfig,
        instance_id: Uuid,
        initial_config: Option<String>,
    ) -> Result<Uuid, String> {
        crate::log_info!(format!("EasyTierManager: 启动网络实例, network_name={}, id={}", cfg.network_name, instance_id));
        let running = launcher::start_easytier(&cfg, instance_id, &self.config_dir, initial_config).await?;
        self.instances.insert(instance_id, running);
        crate::log_info!(format!("EasyTierManager: 网络实例已启动, id={}", instance_id));
        Ok(instance_id)
    }

    /// 停止网络实例，返回最新配置内容（TOML 字符串）
    pub async fn stop_network(&self, instance_id: &Uuid) -> Result<Option<String>, String> {
        crate::log_info!(format!("EasyTierManager: 停止网络实例, id={}", instance_id));
        if let Some((_, mut instance)) = self.instances.remove(instance_id) {
            let config = instance.stop().await?;
            crate::log_info!(format!("EasyTierManager: 网络实例已停止, id={}", instance_id));
            Ok(config)
        } else {
            crate::log_warn!(format!("EasyTierManager: 实例未找到, id={}", instance_id));
            Ok(None)
        }
    }

    /// 获取网络状态
    pub async fn get_status(&self, instance_id: &Uuid) -> Result<NetworkStatus, String> {
        let instance = self.instances.get(instance_id)
            .ok_or_else(|| {
                crate::log_warn!(format!("EasyTierManager: 获取状态失败, 实例未找到, id={}", instance_id));
                "Instance not found".to_string()
            })?;
        instance.get_status().await
    }

    /// 获取连接的 peer 数量
    pub fn get_connected_peers(&self, instance_id: &Uuid) -> Option<u32> {
        self.instances.get(instance_id).and_then(|inst| inst.connected_peers())
    }

    /// 获取 peer 列表
    pub async fn get_peers(&self, instance_id: &Uuid) -> Result<Vec<crate::easytier::launcher::PeerInfo>, String> {
        let instance = self.instances.get(instance_id)
            .ok_or_else(|| "Instance not found".to_string())?;
        Ok(instance.get_peers().await)
    }

    /// 获取虚拟 IP
    pub fn get_virtual_ip(&self, instance_id: &Uuid) -> Option<String> {
        self.instances.get(instance_id).and_then(|inst| inst.virtual_ip())
    }

    /// 检查网络是否正在运行
    pub fn is_running(&self, instance_id: &Uuid) -> bool {
        self.instances.contains_key(instance_id)
    }

    /// 获取所有运行的实例 ID
    pub fn list_running(&self) -> Vec<Uuid> {
        self.instances.iter().map(|e| *e.key()).collect()
    }
}