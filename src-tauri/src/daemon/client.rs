use super::ipc::{IpcRequest, IpcResponse, DEFAULT_RPC_PORT};
use std::sync::OnceLock;
use std::time::Duration;

/// 全局共享 IPC 客户端（所有命令 handler 共享同一长连接）
static GLOBAL_IPC: OnceLock<IpcClient> = OnceLock::new();

/// TCP RPC 客户端
pub struct IpcClient {
    port: u16,
    stream: tokio::sync::Mutex<Option<tokio::net::TcpStream>>,
}

impl IpcClient {
    /// 创建新的 IPC 客户端
    pub fn new(port: u16) -> Self {
        Self { port, stream: tokio::sync::Mutex::new(None) }
    }

    /// 创建默认端口的客户端
    pub fn default_port() -> Self {
        Self::new(DEFAULT_RPC_PORT)
    }

    /// 关闭长连接
    pub async fn close(&self) {
        *self.stream.lock().await = None;
    }

    /// 发送请求到 daemon（复用长连接，断线自动重连）
    pub async fn send(&self, request: &IpcRequest) -> Result<IpcResponse, String> {
        let addr = format!("127.0.0.1:{}", self.port);
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::time::timeout;

        let mut stream_guard = self.stream.lock().await;

        // 确保有可用连接
        if stream_guard.is_none() {
            let stream = timeout(Duration::from_secs(5), tokio::net::TcpStream::connect(&addr)).await
                .map_err(|_| "连接 daemon 超时".to_string())?
                .map_err(|e| format!("连接 daemon 失败: {}", e))?;
            *stream_guard = Some(stream);
        }

        // 序列化请求
        let req_json = serde_json::to_string(request)
            .map_err(|e| format!("序列化请求失败: {}", e))?;
        let len = req_json.len() as u32;

        let stream = stream_guard.as_mut().unwrap();

        // 发送长度 + 内容
        if let Err(e) = async {
            stream.write_all(&len.to_le_bytes()).await?;
            stream.write_all(req_json.as_bytes()).await?;
            Ok::<_, std::io::Error>(())
        }.await {
            *stream_guard = None;
            return Err(format!("发送请求失败: {}", e));
        }

        // 读取响应长度（30s 超时）
        let mut len_buf = [0u8; 4];
        match timeout(Duration::from_secs(30), stream.read_exact(&mut len_buf)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                *stream_guard = None;
                return Err(format!("读取响应长度失败: {}", e));
            }
            Err(_) => {
                *stream_guard = None;
                return Err("读取响应长度超时".to_string());
            }
        }
        let resp_len = u32::from_le_bytes(len_buf) as usize;

        if resp_len > 10 * 1024 * 1024 {
            *stream_guard = None;
            return Err("响应过大".into());
        }

        // 读取响应内容（30s 超时）
        let mut resp_buf = vec![0u8; resp_len];
        match timeout(Duration::from_secs(30), stream.read_exact(&mut resp_buf)).await {
            Ok(Ok(_)) => {
                serde_json::from_slice(&resp_buf)
                    .map_err(|e| format!("反序列化响应失败: {}", e))
            }
            Ok(Err(e)) => {
                *stream_guard = None;
                Err(format!("读取响应内容失败: {}", e))
            }
            Err(_) => {
                *stream_guard = None;
                Err("读取响应内容超时".to_string())
            }
        }
    }

    /// 同步 Ping daemon（用于 setup 等非 async 上下文）
    pub fn ping_sync(&self) -> bool {
        let addr = format!("127.0.0.1:{}", self.port);
        if let Ok(mut stream) = std::net::TcpStream::connect_timeout(
            &addr.parse().unwrap_or(std::net::SocketAddr::from(([127, 0, 0, 1], self.port))),
            Duration::from_secs(5),
        ) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
            let req_json = serde_json::to_string(&IpcRequest::Ping).ok();
            if let Some(json) = req_json {
                let len = json.len() as u32;
                use std::io::{Read, Write};
                if stream.write_all(&len.to_le_bytes()).is_err() { return false; }
                if stream.write_all(json.as_bytes()).is_err() { return false; }
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).is_err() { return false; }
                let _resp_len = u32::from_le_bytes(len_buf) as usize;
                return true;
            }
        }
        false
    }

    /// Ping daemon
    pub async fn ping(&self) -> bool {
        self.send(&IpcRequest::Ping).await.is_ok()
    }

    /// 获取 daemon 状态
    pub async fn get_status(&self) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::GetStatus).await
    }

    /// 连接 space
    pub async fn connect_space(&self, space_id: &str, config: serde_json::Value) -> Result<IpcResponse, String> {
        crate::log_info!(format!("[IpcClient] connect_space 调用, space_id={}, port={}", space_id, self.port));
        let result = self.send(&IpcRequest::ConnectSpace {
            space_id: space_id.to_string(),
            config,
        }).await;
        match &result {
            Ok(resp) => crate::log_info!(format!("[IpcClient] connect_space 响应: {:?}", resp)),
            Err(e) => crate::log_error!(format!("[IpcClient] connect_space 失败: {}", e)),
        }
        result
    }

    /// 断开 space
    pub async fn disconnect_space(&self, space_id: &str) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::DisconnectSpace {
            space_id: space_id.to_string(),
        }).await
    }

    /// 列出 spaces
    pub async fn list_spaces(&self) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::ListSpaces).await
    }

    /// 获取版本
    pub async fn get_version(&self) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::GetVersion).await
    }

    /// 获取空间运行时状态（通过 RPC 查询）
    pub async fn get_space_status(&self, space_id: &str) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::GetSpaceStatus {
            space_id: space_id.to_string(),
        }).await
    }

    /// 列出 peers
    pub async fn list_peers(&self, space_id: &str) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::ListPeers {
            space_id: space_id.to_string(),
        }).await
    }

    /// 运行时修改空间配置
    pub async fn patch_config(&self, space_id: &str, patch: serde_json::Value) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::PatchConfig {
            space_id: space_id.to_string(),
            patch,
        }).await
    }

    /// 升级版本
    pub async fn upgrade(&self, version: &str, source_path: Option<&str>) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::UpgradeVersion {
            version: version.to_string(),
            source_path: source_path.map(|s| s.to_string()),
        }).await
    }

    /// 切换二进制（升级后通知 daemon 重启运行中的实例）
    pub async fn switch_binary(&self) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::SwitchBinary).await
    }

    /// 同步关闭 daemon（用于 setup 等非 async 上下文）
    pub fn shutdown_sync(&self) -> bool {
        self.send_sync(&IpcRequest::Shutdown).is_ok()
    }

    /// 同步发送 IPC 请求（通用，用于非 async 上下文）
    pub fn send_sync(&self, request: &IpcRequest) -> Result<IpcResponse, String> {
        let addr = format!("127.0.0.1:{}", self.port);
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect_timeout(
            &addr.parse().unwrap_or(std::net::SocketAddr::from(([127, 0, 0, 1], self.port))),
            Duration::from_secs(5),
        ).map_err(|e| format!("连接 daemon 失败: {}", e))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("设置读超时失败: {}", e))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| format!("设置写超时失败: {}", e))?;

        let req_json = serde_json::to_string(request)
            .map_err(|e| format!("序列化请求失败: {}", e))?;
        let len = req_json.len() as u32;

        stream.write_all(&len.to_le_bytes())
            .map_err(|e| format!("发送请求长度失败: {}", e))?;
        stream.write_all(req_json.as_bytes())
            .map_err(|e| format!("发送请求内容失败: {}", e))?;

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)
            .map_err(|e| format!("读取响应长度失败: {}", e))?;
        let resp_len = u32::from_le_bytes(len_buf) as usize;

        if resp_len > 10 * 1024 * 1024 {
            return Err("响应过大".into());
        }

        let mut resp_buf = vec![0u8; resp_len];
        stream.read_exact(&mut resp_buf)
            .map_err(|e| format!("读取响应内容失败: {}", e))?;

        serde_json::from_slice(&resp_buf)
            .map_err(|e| format!("反序列化响应失败: {}", e))
    }

    /// 获取 daemon 日志（增量，可选按 space_id 过滤）
    pub async fn get_logs(&self, level: Option<&str>, since_seq: Option<u64>, space_id: Option<&str>) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::GetLogs {
            level: level.map(|s| s.to_string()),
            since_seq,
            space_id: space_id.map(|s| s.to_string()),
        }).await
    }

    /// 转发日志条目到 daemon（GUI 进程使用）
    pub async fn write_log(&self, entries: Vec<crate::log::LogEntry>) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::WriteLog { entries }).await
    }

    /// 清空 daemon 日志
    pub async fn clear_daemon_logs(&self) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::ClearDaemonLogs).await
    }

    /// 检查 EasyTier 二进制
    pub async fn check_binary(&self) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::CheckBinary).await
    }

    /// 关闭 daemon
    pub async fn shutdown(&self) -> Result<IpcResponse, String> {
        self.send(&IpcRequest::Shutdown).await
    }

    /// 获取全局 IPC 客户端实例（所有命令 handler 共享同一长连接）
    pub fn get_global() -> &'static IpcClient {
        GLOBAL_IPC.get_or_init(IpcClient::default_port)
    }
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::default_port()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = IpcClient::new(DEFAULT_RPC_PORT);
        assert_eq!(client.port, DEFAULT_RPC_PORT);
    }
}
