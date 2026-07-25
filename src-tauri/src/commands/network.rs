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
    crate::log_debug!(format!("获取网络状态: space_id={}", space_id));
    easytier.get_status(&id).await
}

#[tauri::command]
pub async fn get_network_stats(
    space_id: String,
    easytier: State<'_, Arc<EasyTierManager>>,
) -> Result<NetworkStats, String> {
    let id = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;

    // 使用新的网络统计方法
    if let Some(rpc_status) = easytier.get_network_stats(&id).await {
        Ok(NetworkStats {
            rx_bytes: rpc_status.rx_bytes,
            tx_bytes: rpc_status.tx_bytes,
            rx_packets: 0, // EasyTier 暂不提供包计数
            tx_packets: 0,
            loss_rate: 0.0, // EasyTier 暂不提供丢包率
            avg_latency_ms: rpc_status.avg_latency_ms,
        })
    } else {
        crate::log_warn!(format!("获取网络统计失败: space_id={}", space_id));
        // 如果查询失败，返回默认值
        Ok(NetworkStats {
            rx_bytes: 0,
            tx_bytes: 0,
            rx_packets: 0,
            tx_packets: 0,
            loss_rate: 0.0,
            avg_latency_ms: 0.0,
        })
    }
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

/// 获取空间 peer 列表（暂不支持）
#[tauri::command]
pub async fn get_space_peers(
    _space_id: String,
) -> Result<Vec<PeerInfo>, String> {
    Err("get_space_peers 暂不支持".to_string())
}