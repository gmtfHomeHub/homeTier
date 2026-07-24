use tauri::State;
use crate::types::{NetworkStatus, NetworkStats};
use crate::easytier::config::NetworkConfig;
use crate::easytier::{EasyTierManager, launcher::PeerInfo};
use crate::space::manager::SpaceManager;
use std::sync::Arc;

#[tauri::command]
pub async fn get_network_status(
    space_id: String,
    easytier: State<'_, Arc<EasyTierManager>>,
) -> Result<NetworkStatus, String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    easytier.get_status(&id).await
}

#[tauri::command]
pub async fn get_network_stats(
    space_id: String,
    easytier: State<'_, Arc<EasyTierManager>>,
) -> Result<NetworkStats, String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;

    // 从 EasyTier RPC 获取统计数据
    let status = easytier.get_status(&id).await?;

    // 构造统计数据
    Ok(NetworkStats {
        rx_bytes: 0,
        tx_bytes: 0,
        rx_packets: 0,
        tx_packets: 0,
        loss_rate: 0.0,
        avg_latency_ms: status.latency_ms.unwrap_or(0.0),
    })
}

#[tauri::command]
pub async fn update_group_config(
    space_id: String,
    config: NetworkConfig,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;

    // 更新群配置并同步到所有成员
    crate::log_info!(format!("更新群配置: space_id={}", space_id), &space_id);

    // 断开当前连接，使用新配置重新连接
    space_manager.disconnect(&id).await?;
    space_manager.connect(&id).await?;

    Ok(())
}

#[tauri::command]
pub async fn update_local_config(
    space_id: String,
    _config: NetworkConfig,
) -> Result<(), String> {
    // 更新本地配置
    crate::log_info!(format!("更新本地配置: space_id={}", space_id), &space_id);

    // TODO: 将本地配置持久化到数据库
    // 后续启动时使用本地配置覆盖群配置

    Ok(())
}

#[tauri::command]
pub async fn get_effective_config(
    space_id: String,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<NetworkConfig, String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;

    // 从 space 中获取当前的网络配置
    let spaces = space_manager.list().await?;
    let space = spaces.iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "Space not found".to_string())?;

    Ok(NetworkConfig {
        network_name: space.network_name.clone(),
        network_secret: space.network_secret.clone(),
        dhcp: true,
        ..Default::default()
    })
}

/// 获取空间 peer 列表（暂不支持）
#[tauri::command]
pub async fn get_space_peers(
    _space_id: String,
) -> Result<Vec<PeerInfo>, String> {
    Err("get_space_peers 暂不支持".to_string())
}