use serde::{Deserialize, Serialize};

/// 默认 RPC 端口（可通过配置文件 DAEMON_IPC_PORT 覆盖，下次 daemon 启动生效）
pub const DEFAULT_RPC_PORT: u16 = 15889;
/// easytier-core daemon 的 RPC 端口（可通过配置文件 EASYTIER_RPC_PORT 覆盖，下次 daemon 启动生效）
pub const EASYTIER_DAEMON_RPC_PORT: u16 = 15888;

/// 读取配置后的 homeTier daemon IPC 端口（回退默认值）
pub fn default_rpc_port() -> u16 {
    crate::config::get_u16(crate::config::KEY_DAEMON_IPC_PORT, DEFAULT_RPC_PORT)
}

/// 读取配置后的 easytier-core RPC 端口（回退默认值）
pub fn easytier_daemon_rpc_port() -> u16 {
    crate::config::get_u16(crate::config::KEY_EASYTIER_RPC_PORT, EASYTIER_DAEMON_RPC_PORT)
}

/// IPC 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    Ping,
    GetStatus,
    ConnectSpace {
        space_id: String,
        config: serde_json::Value,
    },
    DisconnectSpace {
        space_id: String,
    },
    GetSpaceStatus {
        space_id: String,
    },
    ListPeers {
        space_id: String,
    },
    PatchConfig {
        space_id: String,
        patch: serde_json::Value,
    },
    ListSpaces,
    GetVersion,
    UpgradeVersion {
        version: String,
        source_path: Option<String>,
    },
    SwitchBinary,
    GetLogs {
        level: Option<String>,
        since_seq: Option<u64>,
        space_id: Option<String>,
    },
    WriteLog {
        entries: Vec<crate::log::LogEntry>,
    },
    ClearDaemonLogs,
    SetLogEnabled {
        enabled: bool,
    },
    CheckBinary,
    Shutdown,
}

/// IPC 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    Ok { data: Option<serde_json::Value> },
    Error { message: String },
}

/// Daemon 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: u32,
    pub connected_spaces: Vec<String>,
    pub version: String,
    pub rpc_port: u16,
}

/// 空间运行时状态（通过 RPC 查询）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceRuntimeStatus {
    pub space_id: String,
    pub is_running: bool,
    pub virtual_ip: Option<String>,
    pub connected_peers: u32,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub avg_latency_ms: f64,
}


