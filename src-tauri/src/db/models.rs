use serde::{Deserialize, Serialize};

/// 用户行数据（数据库映射，单行表：本机用户）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRow {
    pub id: String, // machine_id
    pub name: String, // hostname
    pub config_json: Option<String>,
}

/// 空间行数据（数据库映射）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceRow {
    pub id: String,
    pub name: String,
    pub owner_id: Option<String>,
    pub network_name: String,
    pub network_secret: String,
    pub description: Option<String>,
    pub created_at: String,
    pub last_connected_at: Option<String>,
    pub is_auto_connect: bool,
    pub config_json: Option<String>,
}

/// 消息行数据（数据库映射）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRow {
    pub id: String,
    pub space_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub msg_type: String,
    pub content: String,
    pub timestamp: String,
    pub status: String,
}

/// 成员行数据（数据库映射）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRow {
    pub id: String,
    pub space_id: String,
    pub nickname: String,
    pub virtual_ip: Option<String>,
    pub is_online: bool,
    pub is_owner: bool,
    pub joined_at: String,
    pub last_seen_at: Option<String>,
}

/// 应用行数据（数据库映射）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRow {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub protocol: Option<String>,
    pub hostname: Option<String>,
    pub port: Option<String>,
    pub pathname: Option<String>,
    pub sort_order: i32,
    pub created_by: String,
    pub created_at: String,
}

/// 文件行数据（数据库映射）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRow {
    pub id: String,
    pub space_id: String,
    pub sender_id: String,
    pub file_name: String,
    pub file_size: i64,
    pub file_hash: Option<String>,
    pub mime_type: Option<String>,
    pub is_compressed: bool,
    pub is_password_protected: bool,
    pub storage_path: Option<String>,
    pub created_at: String,
}

/// ACL 规则行数据（数据库映射）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRuleRow {
    pub id: String,
    pub space_id: String,
    pub action: String, // "allow" 或 "deny"
    pub source: String,
    pub dest: String,
    pub ports: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 端口转发规则行数据（数据库映射）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForwardRuleRow {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub protocol: String, // "tcp" 或 "udp"
    pub source_ip: String,
    pub source_port: i32,
    pub target_ip: String,
    pub target_port: i32,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}
