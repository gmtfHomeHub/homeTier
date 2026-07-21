use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 空间信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Option<String>,
    pub network_name: String,
    pub network_secret: String,
    pub created_at: chrono::DateTime<chrono::Local>,
    pub last_connected_at: Option<chrono::DateTime<chrono::Local>>,
    pub is_auto_connect: bool,
    pub status: SpaceStatus,
    pub virtual_ip: Option<String>,
    pub member_count: u32,
    pub config_json: Option<String>,
}

/// 空间连接状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpaceStatus {
    Disconnected,
    Connecting,
    Connected,
}

/// 空间成员
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: Uuid,
    pub space_id: Uuid,
    pub nickname: String,
    pub virtual_ip: Option<String>,
    pub is_online: bool,
    pub is_owner: bool,
    pub joined_at: chrono::DateTime<chrono::Local>,
    pub last_seen_at: Option<chrono::DateTime<chrono::Local>>,
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub space_id: Uuid,
    pub sender_id: Uuid,
    pub sender_name: String,
    pub msg_type: MessageType,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub status: MessageStatus,
}

/// 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Text,
    Image,
    System,
}

/// 消息状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageStatus {
    Sending,
    Sent,
    Delivered,
    Failed,
}

/// 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: Uuid,
    pub space_id: Uuid,
    pub sender_id: Uuid,
    pub file_name: String,
    pub file_size: u64,
    pub file_hash: Option<String>,
    pub mime_type: Option<String>,
    pub is_compressed: bool,
    pub is_password_protected: bool,
    pub storage_path: Option<String>,
    pub created_at: chrono::DateTime<chrono::Local>,
}

/// 传输进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub transfer_id: Uuid,
    pub file_name: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub status: TransferStatus,
}

/// 传输状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferStatus {
    Transferring,
    Paused,
    Completed,
    Failed,
}

/// 网络状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub space_id: Uuid,
    pub status: String,
    pub virtual_ip: Option<String>,
    pub latency_ms: Option<f64>,
    pub connected_peers: u32,
}

/// 网络统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub loss_rate: f64,
    pub avg_latency_ms: f64,
}

/// 分享链接信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInfo {
    pub network_name: String,
    pub network_secret: String,
    pub host_hint: Option<String>,
}

/// 快捷键信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyInfo {
    pub key: String,
    pub action: String,
    pub enabled: bool,
}

/// 端口转发规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForwardRule {
    pub bind_ip: String,
    pub bind_port: u16,
    pub dst_ip: String,
    pub dst_port: u16,
    pub proto: String,
}

/// ACL 规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    pub action: String, // allow / deny
    pub cidr: String,
    pub port_range: Option<String>,
    pub proto: Option<String>,
}
