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

/// ACL 规则（与 db/models.rs AclRuleRow 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    pub id: String,
    pub space_id: String,
    pub action: String,
    pub source: String,
    pub dest: String,
    pub ports: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 端口转发规则（与 db/models.rs PortForwardRuleRow 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForwardRule {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub protocol: String,
    pub source_ip: String,
    pub source_port: i32,
    pub target_ip: String,
    pub target_port: i32,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

/// TUN 授权结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub success: bool,
    pub message: String,
    pub needs_restart: bool,
}

/// TUN 能力状态查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunStatus {
    pub tun_available: bool,
    pub platform: &'static str,
    pub elevated: bool,
}
