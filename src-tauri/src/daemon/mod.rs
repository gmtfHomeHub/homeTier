pub mod client;
pub mod ipc;
pub mod service;

use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use crate::easytier::EasyTierManager;

/// Daemon 核心结构（参考 EasyTier daemon 模式）
pub struct Daemon {
    status: Arc<RwLock<ipc::DaemonStatus>>,
    easytier: Arc<EasyTierManager>,
    rpc_port: u16,
    shutdown_tx: broadcast::Sender<()>,
}

impl Daemon {
    pub fn new() -> Result<Self, String> {
        let config_dir = Self::get_config_dir()?;
        let easytier_dir = config_dir.join("easytier");
        std::fs::create_dir_all(&easytier_dir)
            .map_err(|e| format!("创建 EasyTier 配置目录失败: {}", e))?;

        let easytier = Arc::new(EasyTierManager::new(easytier_dir));
        let (shutdown_tx, _) = broadcast::channel(1);

        let status = ipc::DaemonStatus {
            running: true,
            pid: std::process::id(),
            connected_spaces: Vec::new(),
            version: env!("CARGO_PKG_VERSION").into(),
            rpc_port: ipc::DEFAULT_RPC_PORT,
        };

        Ok(Self {
            status: Arc::new(RwLock::new(status)),
            easytier,
            rpc_port: ipc::DEFAULT_RPC_PORT,
            shutdown_tx,
        })
    }

    fn get_config_dir() -> Result<std::path::PathBuf, String> {
        directories::BaseDirs::new()
            .map(|d| d.config_dir().join("homeTier"))
            .ok_or_else(|| "无法获取配置目录".into())
    }

    /// daemon 主循环（参考 EasyTier daemon 模式：block_on, 监听 ctrl_c）
    pub async fn run(&self) -> Result<(), String> {
        crate::log_info!("[Daemon] 守护进程启动");

        // 保存状态到文件（供 GUI 检测）
        ipc::save_daemon_state(self.rpc_port.into(), self.rpc_port)?;

        // 启动 TCP RPC 服务器
        let addr = format!("127.0.0.1:{}", self.rpc_port);
        let listener = tokio::net::TcpListener::bind(&addr).await
            .map_err(|e| format!("绑定 TCP 端口失败: {}", e))?;

        crate::log_info!(format!("[Daemon] TCP RPC 服务器已启动: {}", addr));

        // 监听 ctrl_c 信号（参考 EasyTier stop_check_notifier）
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let shutdown_tx = self.shutdown_tx.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            crate::log_info!("[Daemon] 收到 ctrl_c 信号，准备关闭");
            let _ = shutdown_tx.send(());
        });

        // 主循环：接受连接 + 监听 shutdown
        loop {
            tokio::select! {
                Ok((stream, peer_addr)) = listener.accept() => {
                    crate::log_debug!(format!("[Daemon] 新连接: {}", peer_addr));
                    let status = self.status.clone();
                    let easytier = self.easytier.clone();
                    let shutdown_tx = self.shutdown_tx.clone();
                    tokio::spawn(async move {
                        Self::handle_connection(stream, status, easytier, shutdown_tx).await;
                    });
                }
                _ = shutdown_rx.recv() => {
                    crate::log_info!("[Daemon] 收到关闭信号，停止所有实例");
                    self.stop_all().await;
                    ipc::clear_daemon_state();
                    break;
                }
            }
        }

        crate::log_info!("[Daemon] 守护进程已退出");
        Ok(())
    }

    /// 处理 TCP 连接
    async fn handle_connection(
        mut stream: tokio::net::TcpStream,
        status: Arc<RwLock<ipc::DaemonStatus>>,
        easytier: Arc<EasyTierManager>,
        shutdown_tx: broadcast::Sender<()>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        loop {
            // 读取请求长度 (4 bytes)
            let mut len_buf = [0u8; 4];
            match stream.read_exact(&mut len_buf).await {
                Ok(_) => {},
                Err(_) => break,
            }
            let req_len = u32::from_le_bytes(len_buf) as usize;

            if req_len > 10 * 1024 * 1024 { // 10MB 限制
                crate::log_warn!("[Daemon] 请求过大");
                break;
            }

            // 读取请求内容
            let mut req_buf = vec![0u8; req_len];
            if stream.read_exact(&mut req_buf).await.is_err() {
                break;
            }

            // 解析请求
            let req: ipc::IpcRequest = match serde_json::from_slice(&req_buf) {
                Ok(r) => r,
                Err(e) => {
                    let resp = ipc::IpcResponse::Error { message: format!("解析请求失败: {}", e) };
                    Self::send_response(&mut stream, &resp).await;
                    continue;
                }
            };

            // 处理请求
            let resp = Self::handle_request(req, &status, &easytier, &shutdown_tx).await;

            // 发送响应
            Self::send_response(&mut stream, &resp).await;
        }
    }

    /// 处理 IPC 请求
    async fn handle_request(
        req: ipc::IpcRequest,
        status: &Arc<RwLock<ipc::DaemonStatus>>,
        easytier: &Arc<EasyTierManager>,
        shutdown_tx: &broadcast::Sender<()>,
    ) -> ipc::IpcResponse {
        match req {
            ipc::IpcRequest::Ping => {
                ipc::IpcResponse::Ok { data: None }
            }
            ipc::IpcRequest::GetStatus => {
                let s = status.read().await;
                match serde_json::to_value(&*s) {
                    Ok(v) => ipc::IpcResponse::Ok { data: Some(v) },
                    Err(e) => ipc::IpcResponse::Error { message: format!("序列化状态失败: {}", e) },
                }
            }
            ipc::IpcRequest::ConnectSpace { space_id, config } => {
                crate::log_info!(format!("[Daemon] 连接空间: {}", space_id));

                // 解析配置
                let network_config: crate::easytier::config::NetworkConfig = match serde_json::from_value(config) {
                    Ok(c) => c,
                    Err(e) => return ipc::IpcResponse::Error { message: format!("解析配置失败: {}", e) },
                };

                // 启动 easytier
                let instance_id = match uuid::Uuid::parse_str(&space_id) {
                    Ok(id) => id,
                    Err(_) => return ipc::IpcResponse::Error { message: "无效的 space_id".into() },
                };

                match easytier.start_network(&network_config, instance_id, None).await {
                    Ok(id) => {
                        let mut s = status.write().await;
                        if !s.connected_spaces.contains(&space_id) {
                            s.connected_spaces.push(space_id);
                        }
                        ipc::IpcResponse::Ok { data: Some(serde_json::json!({ "instance_id": id.to_string() })) }
                    }
                    Err(e) => ipc::IpcResponse::Error { message: format!("连接空间失败: {}", e) },
                }
            }
            ipc::IpcRequest::DisconnectSpace { space_id } => {
                crate::log_info!(format!("[Daemon] 断开空间: {}", space_id));
                match uuid::Uuid::parse_str(&space_id) {
                    Ok(id) => {
                        match easytier.stop_network(&id).await {
                            Ok(_) => {
                                let mut s = status.write().await;
                                s.connected_spaces.retain(|id| id != &space_id);
                                ipc::IpcResponse::Ok { data: None }
                            }
                            Err(e) => ipc::IpcResponse::Error { message: format!("断开空间失败: {}", e) },
                        }
                    }
                    Err(e) => ipc::IpcResponse::Error { message: format!("无效的 space_id: {}", e) },
                }
            }
            ipc::IpcRequest::GetSpaceStatus { space_id } => {
                crate::log_debug!(format!("[Daemon] 查询空间状态: {}", space_id));
                match uuid::Uuid::parse_str(&space_id) {
                    Ok(id) => {
                        match easytier.get_space_status(&id).await {
                            Some(status) => {
                                match serde_json::to_value(&status) {
                                    Ok(v) => ipc::IpcResponse::Ok { data: Some(v) },
                                    Err(e) => ipc::IpcResponse::Error { message: format!("序列化状态失败: {}", e) },
                                }
                            }
                            None => {
                                ipc::IpcResponse::Ok { data: Some(serde_json::json!({
                                    "space_id": space_id,
                                    "is_running": false,
                                    "virtual_ip": null,
                                    "connected_peers": 0,
                                    "rx_bytes": 0,
                                    "tx_bytes": 0,
                                    "avg_latency_ms": 0.0,
                                }))}
                            }
                        }
                    }
                    Err(e) => ipc::IpcResponse::Error { message: format!("无效的 space_id: {}", e) },
                }
            }
            ipc::IpcRequest::ListPeers { space_id } => {
                crate::log_debug!(format!("[Daemon] 查询 peer 列表: {}", space_id));
                match uuid::Uuid::parse_str(&space_id) {
                    Ok(id) => {
                        match easytier.get_peers(&id).await {
                            Ok(peers) => {
                                match serde_json::to_value(&peers) {
                                    Ok(v) => ipc::IpcResponse::Ok { data: Some(v) },
                                    Err(e) => ipc::IpcResponse::Error { message: format!("序列化 peer 列表失败: {}", e) },
                                }
                            }
                            Err(e) => ipc::IpcResponse::Error { message: format!("查询 peer 列表失败: {}", e) },
                        }
                    }
                    Err(e) => ipc::IpcResponse::Error { message: format!("无效的 space_id: {}", e) },
                }
            }
            ipc::IpcRequest::PatchConfig { space_id, patch } => {
                crate::log_info!(format!("[Daemon] 修改空间配置: {}", space_id));
                match uuid::Uuid::parse_str(&space_id) {
                    Ok(id) => {
                        match easytier.patch_config(&id, &patch).await {
                            Ok(()) => {
                                // 更新状态中的 connected_spaces
                                let mut s = status.write().await;
                                if !s.connected_spaces.contains(&space_id) {
                                    s.connected_spaces.push(space_id);
                                }
                                ipc::IpcResponse::Ok { data: None }
                            }
                            Err(e) => ipc::IpcResponse::Error { message: format!("修改配置失败: {}", e) },
                        }
                    }
                    Err(e) => ipc::IpcResponse::Error { message: format!("无效的 space_id: {}", e) },
                }
            }
            ipc::IpcRequest::ListSpaces => {
                let spaces = easytier.list_saved();
                match serde_json::to_value(&spaces) {
                    Ok(v) => ipc::IpcResponse::Ok { data: Some(v) },
                    Err(e) => ipc::IpcResponse::Error { message: format!("序列化失败: {}", e) },
                }
            }
            ipc::IpcRequest::GetVersion => {
                match easytier.get_version().await {
                    Ok(v) => ipc::IpcResponse::Ok { data: Some(serde_json::json!({ "version": v })) },
                    Err(e) => ipc::IpcResponse::Error { message: e },
                }
            }
            ipc::IpcRequest::UpgradeVersion { version, source_path } => {
                crate::log_info!(format!("[Daemon] 升级版本: {}", version));
                let source = source_path.map(|path| crate::easytier::BinarySource::LocalBinary(std::path::PathBuf::from(path)));
                match easytier.upgrade(&version, source).await {
                    Ok(()) => ipc::IpcResponse::Ok { data: None },
                    Err(e) => ipc::IpcResponse::Error { message: format!("升级失败: {}", e) },
                }
            }
            ipc::IpcRequest::Shutdown => {
                crate::log_info!("[Daemon] 收到关闭命令");
                let _ = shutdown_tx.send(());
                ipc::IpcResponse::Ok { data: None }
            }
        }
    }

    /// 发送 IPC 响应
    async fn send_response(
        stream: &mut tokio::net::TcpStream,
        resp: &ipc::IpcResponse,
    ) {
        use tokio::io::AsyncWriteExt;
        if let Ok(msg) = serde_json::to_string(resp) {
            let len = msg.len() as u32;
            let _ = stream.write_all(&len.to_le_bytes()).await;
            let _ = stream.write_all(msg.as_bytes()).await;
        }
    }

    /// 停止所有实例
    async fn stop_all(&self) {
        let running = self.easytier.list_running();
        for id in running {
            let _ = self.easytier.stop_network(&id).await;
        }
        let mut s = self.status.write().await;
        s.connected_spaces.clear();
        s.running = false;
    }
}

/// daemon 入口点（从 main.rs 调用）
pub async fn run_daemon_async() -> Result<(), String> {
    let daemon = Daemon::new()?;
    daemon.run().await
}
