use super::ipc::{IpcCommand, IpcResponse};
use std::path::PathBuf;
use std::time::Duration;

/// IPC 客户端
pub struct IpcClient {
    socket_path: PathBuf,
}

impl IpcClient {
    /// 创建新的 IPC 客户端
    pub fn new() -> Self {
        Self {
            socket_path: super::ipc::get_daemon_socket_path(),
        }
    }

    /// 发送命令到守护进程
    pub fn send_command(&self, cmd: &IpcCommand) -> Result<IpcResponse, String> {
        let stream = std::os::unix::net::UnixStream::connect(&self.socket_path)
            .map_err(|e| format!("连接守护进程失败: {}", e))?;

        stream.set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("设置读超时失败: {}", e))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("设置写超时失败: {}", e))?;

        // 发送命令
        let msg = serde_json::to_string(cmd)
            .map_err(|e| format!("序列化命令失败: {}", e))?;
        let len = msg.len() as u32;

        use std::io::Write;
        let mut stream = stream;
        stream.write_all(&len.to_le_bytes())
            .map_err(|e| format!("发送命令长度失败: {}", e))?;
        stream.write_all(msg.as_bytes())
            .map_err(|e| format!("发送命令内容失败: {}", e))?;

        // 读取响应
        let mut len_buf = [0u8; 4];
        use std::io::Read;
        stream.read_exact(&mut len_buf)
            .map_err(|e| format!("读取响应长度失败: {}", e))?;
        let resp_len = u32::from_le_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        stream.read_exact(&mut resp_buf)
            .map_err(|e| format!("读取响应内容失败: {}", e))?;

        serde_json::from_slice(&resp_buf)
            .map_err(|e| format!("反序列化响应失败: {}", e))
    }

    /// 检查守护进程是否可达
    pub fn ping(&self) -> bool {
        self.send_command(&IpcCommand::Ping).is_ok()
    }

    /// 获取守护进程状态
    pub fn get_status(&self) -> Result<IpcResponse, String> {
        self.send_command(&IpcCommand::GetStatus)
    }

    /// 连接到空间
    pub fn connect_space(&self, space_id: &str) -> Result<IpcResponse, String> {
        self.send_command(&IpcCommand::ConnectSpace {
            space_id: space_id.to_string(),
            config: None,
        })
    }

    /// 断开空间连接
    pub fn disconnect_space(&self, space_id: &str) -> Result<IpcResponse, String> {
        self.send_command(&IpcCommand::DisconnectSpace {
            space_id: space_id.to_string(),
        })
    }

    /// 获取已连接的空间列表
    pub fn list_spaces(&self) -> Result<IpcResponse, String> {
        self.send_command(&IpcCommand::ListSpaces)
    }

    /// 关闭守护进程
    pub fn shutdown(&self) -> Result<IpcResponse, String> {
        self.send_command(&IpcCommand::Shutdown)
    }
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}
