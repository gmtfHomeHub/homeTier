use serde::{Deserialize, Serialize};

/// 默认 RPC 端口
pub const DEFAULT_RPC_PORT: u16 = 15889;
/// easytier-core daemon 的 RPC 端口（与 homeTier daemon IPC 端口分离）
pub const EASYTIER_DAEMON_RPC_PORT: u16 = 15888;

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


