use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// IPC 命令
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcCommand {
    /// 获取守护进程状态
    GetStatus,
    /// 连接到空间
    ConnectSpace {
        space_id: String,
        config: Option<SpaceConfig>,
    },
    /// 断开空间连接
    DisconnectSpace {
        space_id: String,
    },
    /// 获取已连接的空间列表
    ListSpaces,
    /// 获取网络统计
    GetNetworkStats {
        space_id: String,
    },
    /// 心跳检测
    Ping,
    /// 关闭守护进程
    Shutdown,
}

/// 空间配置（简化版，用于 IPC）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceConfig {
    pub name: Option<String>,
    pub network_key: Option<String>,
    pub subnet: Option<String>,
    pub enable_relay: Option<bool>,
    pub enable_internet: Option<bool>,
}

/// IPC 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    /// 成功
    Ok { data: Option<serde_json::Value> },
    /// 错误
    Error { message: String },
    /// 状态更新（事件推送）
    Event { event: IpcEvent },
}

/// IPC 事件（从守护进程推送到 GUI）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcEvent {
    /// 空间已连接
    SpaceConnected { space_id: String },
    /// 空间已断开
    SpaceDisconnected { space_id: String },
    /// 对等节点上线
    PeerConnected { space_id: String, peer_id: String },
    /// 对等节点下线
    PeerDisconnected { space_id: String, peer_id: String },
    /// 守护进程状态变化
    StatusChanged { status: String },
}

/// 获取守护进程 Unix socket 路径
pub fn get_daemon_socket_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        // Linux: ~/.cache/homeTier/daemon.sock 或 /tmp/homeTier-daemon.sock
        std::env::var("XDG_RUNTIME_DIR")
            .map(|p| PathBuf::from(p).join("hometier-daemon.sock"))
            .unwrap_or_else(|_| PathBuf::from("/tmp").join("hometier-daemon.sock"))
    }
    #[cfg(target_os = "macos")]
    {
        // macOS: /tmp/homeTier-daemon.sock (macOS 没有 XDG_RUNTIME_DIR)
        PathBuf::from("/tmp").join("hometier-daemon.sock")
    }
    #[cfg(target_os = "windows")]
    {
        // Windows: Named pipe, not Unix socket
        // 使用 Windows named pipe: \\.\pipe\hometier-daemon
        PathBuf::from(r"\\.\pipe\hometier-daemon")
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        PathBuf::from("/tmp").join("hometier-daemon.sock")
    }
}

/// 检查守护进程是否正在运行
pub fn is_daemon_running() -> bool {
    let socket_path = get_daemon_socket_path();
    if !socket_path.exists() {
        return false;
    }

    // 尝试连接并发送 Ping 命令
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        match UnixStream::connect(&socket_path) {
            Ok(stream) => {
                stream.set_read_timeout(Some(std::time::Duration::from_secs(1))).ok();
                stream.set_write_timeout(Some(std::time::Duration::from_secs(1))).ok();

                let cmd = IpcCommand::Ping;
                let msg = serde_json::to_string(&cmd).unwrap_or_default();
                let len = msg.len() as u32;
                use std::io::Write;
                let mut stream = stream;
                stream.write_all(&len.to_le_bytes()).ok();
                stream.write_all(msg.as_bytes()).ok();

                // 读取响应
                let mut len_buf = [0u8; 4];
                use std::io::Read;
                stream.read_exact(&mut len_buf).ok();
                let resp_len = u32::from_le_bytes(len_buf) as usize;
                let mut resp_buf = vec![0u8; resp_len];
                stream.read_exact(&mut resp_buf).ok();

                serde_json::from_slice::<IpcResponse>(&resp_buf)
                    .map(|r| matches!(r, IpcResponse::Ok { .. }))
                    .unwrap_or(false)
            }
            Err(_) => false,
        }
    }

    #[cfg(windows)]
    {
        // Windows: 尝试连接 Named Pipe
        false // TODO: 实现 Windows Named Pipe 连接检查
    }
}
