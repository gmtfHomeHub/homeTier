use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    GetDaemonLogs {
        level: Option<String>,
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

/// 获取 daemon 状态文件路径
pub fn get_daemon_state_path() -> PathBuf {
    let app_data = directories::BaseDirs::new()
        .map(|d| d.data_dir().join("com.hometier.app"))
        .unwrap_or_else(|| PathBuf::from("."));
    app_data.join("daemon_state.json")
}

/// 保存 daemon 状态（pid + rpc_port）到文件
pub fn save_daemon_state(pid: u32, rpc_port: u16) -> Result<(), String> {
    let state = serde_json::json!({ "pid": pid, "rpc_port": rpc_port });
    let path = get_daemon_state_path();
    std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap_or_default())
        .map_err(|e| format!("保存 daemon 状态失败: {}", e))
}

/// 读取 daemon 状态
pub fn load_daemon_state() -> Option<(u32, u16)> {
    let path = get_daemon_state_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let pid = val.get("pid")?.as_u64()? as u32;
    let port = val.get("rpc_port")?.as_u64()? as u16;
    Some((pid, port))
}

/// 清除 daemon 状态文件
pub fn clear_daemon_state() {
    let path = get_daemon_state_path();
    std::fs::remove_file(&path).ok();
}

/// 检查指定 pid 的进程是否存活
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use winapi::um::processthreadsapi::{OpenProcess, GetCurrentProcessId};
        use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !handle.is_null() {
                use winapi::um::handleapi::CloseHandle;
                CloseHandle(handle);
                true
            } else {
                false
            }
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

/// 检查 daemon 是否正在运行
pub fn is_daemon_running() -> bool {
    if let Some((pid, _port)) = load_daemon_state() {
        is_process_alive(pid)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_state_path() {
        let path = get_daemon_state_path();
        assert!(path.to_string_lossy().contains("daemon_state.json"));
    }
}
