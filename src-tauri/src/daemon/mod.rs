pub mod client;
pub mod ipc;
pub mod service;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::easytier::EasyTierManager;
use crate::platform::{get_adapter, check_tun_available};
use crate::tun::{get_tun_manager, TunConfig, TunDeviceInfo};

/// 守护进程状态
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: u32,
    pub connected_spaces: Vec<String>,
    pub tun_available: bool,
}

/// 守护进程核心结构
pub struct Daemon {
    status: Arc<RwLock<DaemonStatus>>,
    easytier_manager: Arc<EasyTierManager>,
    config_dir: PathBuf,
}

impl Daemon {
    /// 创建新的守护进程实例
    pub fn new() -> Result<Self, String> {
        let config_dir = Self::get_config_dir()?;
        let easytier_dir = config_dir.join("easytier");
        std::fs::create_dir_all(&easytier_dir)
            .map_err(|e| format!("创建 EasyTier 配置目录失败: {}", e))?;

        let easytier_manager = Arc::new(EasyTierManager::new(easytier_dir));

        let status = DaemonStatus {
            running: true,
            pid: std::process::id(),
            connected_spaces: Vec::new(),
            tun_available: check_tun_available(),
        };

        Ok(Self {
            status: Arc::new(RwLock::new(status)),
            easytier_manager,
            config_dir,
        })
    }

    /// 获取配置目录
    fn get_config_dir() -> Result<PathBuf, String> {
        directories::BaseDirs::new()
            .map(|d| d.config_dir().join("homeTier"))
            .ok_or_else(|| "无法获取配置目录".into())
    }

    /// 启动守护进程主循环
    pub async fn run(&self) -> Result<(), String> {
        log_daemon("守护进程启动");

        // 检查 TUN 能力
        if !self.status.read().await.tun_available {
            log_daemon("警告: TUN 设备不可用，将尝试直接创建");
        }

        // 启动 Unix socket 服务器
        let socket_path = ipc::get_daemon_socket_path();
        self.start_ipc_server(&socket_path).await?;

        log_daemon(&format!("守护进程已启动, PID={}, socket={}", std::process::id(), socket_path.display()));
        Ok(())
    }

    /// 启动 IPC 服务器
    async fn start_ipc_server(&self, socket_path: &PathBuf) -> Result<(), String> {
        // 删除旧的 socket 文件
        if socket_path.exists() {
            std::fs::remove_file(socket_path)
                .map_err(|e| format!("删除旧 socket 失败: {}", e))?;
        }

        #[cfg(unix)]
        {
            use tokio::net::UnixListener;
            let listener = UnixListener::bind(socket_path)
                .map_err(|e| format!("绑定 socket 失败: {}", e))?;

            let status = self.status.clone();
            let easytier = self.easytier_manager.clone();

            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let status = status.clone();
                            let easytier = easytier.clone();
                            tokio::spawn(async move {
                                Self::handle_ipc_connection(stream, status, easytier).await;
                            });
                        }
                        Err(e) => {
                            log_daemon(&format!("接受连接失败: {}", e));
                        }
                    }
                }
            });
        }

        #[cfg(windows)]
        {
            // Windows: 使用 Named Pipe
            // TODO: 实现 Windows Named Pipe 服务器
            log_daemon("Windows Named Pipe 服务器尚未实现");
        }

        Ok(())
    }

    /// 处理 IPC 连接
    async fn handle_ipc_connection(
        stream: tokio::net::UnixStream,
        status: Arc<RwLock<DaemonStatus>>,
        easytier: Arc<EasyTierManager>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut reader, mut writer) = stream.into_split();

        loop {
            // 读取命令长度
            let mut len_buf = [0u8; 4];
            match reader.read_exact(&mut len_buf).await {
                Ok(_) => {},
                Err(_) => break,
            }
            let cmd_len = u32::from_le_bytes(len_buf) as usize;

            // 读取命令内容
            let mut cmd_buf = vec![0u8; cmd_len];
            if reader.read_exact(&mut cmd_buf).await.is_err() {
                break;
            }

            // 解析命令
            let cmd: ipc::IpcCommand = match serde_json::from_slice(&cmd_buf) {
                Ok(c) => c,
                Err(e) => {
                    let resp = ipc::IpcResponse::Error { message: format!("解析命令失败: {}", e) };
                    Self::send_response(&mut writer, &resp).await;
                    continue;
                }
            };

            // 处理命令
            let resp = Self::handle_command(cmd, &status, &easytier).await;

            // 发送响应
            Self::send_response(&mut writer, &resp).await;
        }
    }

    /// 处理 IPC 命令
    async fn handle_command(
        cmd: ipc::IpcCommand,
        status: &Arc<RwLock<DaemonStatus>>,
        easytier: &Arc<EasyTierManager>,
    ) -> ipc::IpcResponse {
        match cmd {
            ipc::IpcCommand::GetStatus => {
                let s = status.read().await;
                match serde_json::to_value(&*s) {
                    Ok(v) => ipc::IpcResponse::Ok { data: Some(v) },
                    Err(e) => ipc::IpcResponse::Error { message: format!("序列化状态失败: {}", e) },
                }
            }
            ipc::IpcCommand::ConnectSpace { space_id, config } => {
                // TODO: 实现空间连接逻辑
                log_daemon(&format!("连接空间: {}", space_id));
                let mut s = status.write().await;
                if !s.connected_spaces.contains(&space_id) {
                    s.connected_spaces.push(space_id.clone());
                }
                ipc::IpcResponse::Ok { data: None }
            }
            ipc::IpcCommand::DisconnectSpace { space_id } => {
                log_daemon(&format!("断开空间: {}", space_id));
                let mut s = status.write().await;
                s.connected_spaces.retain(|id| id != &space_id);
                ipc::IpcResponse::Ok { data: None }
            }
            ipc::IpcCommand::ListSpaces => {
                let s = status.read().await;
                match serde_json::to_value(&s.connected_spaces) {
                    Ok(v) => ipc::IpcResponse::Ok { data: Some(v) },
                    Err(e) => ipc::IpcResponse::Error { message: format!("序列化失败: {}", e) },
                }
            }
            ipc::IpcCommand::GetNetworkStats { space_id } => {
                // TODO: 实现网络统计获取
                ipc::IpcResponse::Ok { data: None }
            }
            ipc::IpcCommand::Ping => {
                ipc::IpcResponse::Ok { data: None }
            }
            ipc::IpcCommand::Shutdown => {
                log_daemon("收到关闭命令");
                std::process::exit(0);
            }
        }
    }

    /// 发送 IPC 响应
    async fn send_response(
        writer: &mut tokio::net::unix::OwnedWriteHalf,
        resp: &ipc::IpcResponse,
    ) {
        use tokio::io::AsyncWriteExt;
        if let Ok(msg) = serde_json::to_string(resp) {
            let len = msg.len() as u32;
            let _ = writer.write_all(&len.to_le_bytes()).await;
            let _ = writer.write_all(msg.as_bytes()).await;
        }
    }
}

fn log_daemon(msg: &str) {
    crate::log_info!("[Daemon] {}", msg);
}

/// 守护进程入口点（从 main.rs 调用）
pub async fn run_daemon_async() -> Result<(), String> {
    let daemon = Daemon::new()?;
    daemon.run().await
}
