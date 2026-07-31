pub mod client;
pub mod ipc;


use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use crate::easytier::EasyTierManager;

async fn wait_rpc_ready(rpc_port: u16, timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match tokio::net::TcpStream::connect(format!("127.0.0.1:{}", rpc_port)).await {
            Ok(_) => {
                crate::log_info!(format!("[Daemon] RPC 端口就绪，耗时 {:?}", start.elapsed()));
                return true;
            }
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
    crate::log_error!(format!("[Daemon] RPC 端口 {} 就绪超时 {:?}", rpc_port, timeout));
    false
}

pub struct Daemon {
    status: Arc<RwLock<ipc::DaemonStatus>>,
    easytier: Arc<EasyTierManager>,
    rpc_port: u16,
    shutdown_tx: broadcast::Sender<()>,
    data_dir: PathBuf,
}

impl Daemon {
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Result<Self, String> {
        // daemon 启动时清空历史日志
        crate::log::clear();
        let easytier_dir = config_dir.join("easytier");
        std::fs::create_dir_all(&easytier_dir)
            .map_err(|e| format!("创建 EasyTier 配置目录失败: {}", e))?;

        let easytier = Arc::new(EasyTierManager::new(easytier_dir, data_dir.clone()));
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
            data_dir,
        })
    }

    /// daemon 主循环（参考 EasyTier daemon 模式：block_on, 监听 ctrl_c）
    pub async fn run(&self) -> Result<(), String> {
        crate::log_info!("[Daemon] 守护进程启动");

        let addr = format!("127.0.0.1:{}", self.rpc_port);

        // 1. 先绑定监听端口（核心能力）
        let listener = tokio::net::TcpListener::bind(&addr).await
            .map_err(|e| {
                crate::log_error!(format!("[Daemon] 绑定端口失败: {}", e));
                format!("绑定 TCP 端口失败: {}", e)
            })?;
        crate::log_info!(format!("[Daemon] TCP RPC 服务器已启动: {}", addr));

        // 2. 写入 signal 文件（GUI 由此确认 daemon 已就绪）
        let signal_path = self.data_dir.join("daemon_ready.signal");
        let _ = std::fs::write(&signal_path, format!("{}", std::process::id()));
        crate::log_info!(format!("[Daemon] signal 文件已写入: {}", signal_path.display()));

        // 3. 离线写状态文件（失败不致命）
        let state_path = self.data_dir.join("daemon_state.json");
        let _ = std::fs::remove_file(&state_path);
        let state_json = serde_json::json!({ "pid": std::process::id(), "rpc_port": self.rpc_port });
        if let Err(e) = std::fs::write(&state_path, serde_json::to_string_pretty(&state_json).unwrap_or_default()) {
            crate::log_info!(format!("[Daemon] daemon_state.json 写入失败（非致命）: {}", e));
        }

        // 启动 easytier-core 守护进程（daemon IPC 就绪后，等待 RPC 端口就绪，再接受 IPC 请求）
        let easytier = self.easytier.clone();
        tokio::spawn(async move {
            crate::log_info!("[Daemon] 正在启动 easytier-core 守护进程...");

            // 清除之前 session 残留的 TOML 配置文件，防止 easytier-core 恢复旧实例
            let config_dir = easytier.get_config_dir();
            crate::log_debug!("[Daemon] 清除 config_dir 中的旧 TOML 文件");
            if let Ok(entries) = std::fs::read_dir(&config_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "toml") {
                        if let Err(e) = std::fs::remove_file(&path) {
                            crate::log_warn!(format!("[Daemon] 删除旧 TOML 文件失败 {}: {}", path.display(), e));
                        } else {
                            crate::log_debug!(format!("[Daemon] 已删除旧 TOML 文件: {}", path.display()));
                        }
                    }
                }
            }
            crate::log_debug!("[Daemon] config_dir 清理完成");

            let binary = match easytier.downloader.ensure_binary().await {
                Ok(b) => b,
                Err(e) => {
                    crate::log_error!(format!("[Daemon] 获取 easytier 二进制失败: {}", e));
                    return;
                }
            };
            crate::log_info!(format!("[Daemon] easytier-core 守护进程 binary={}", binary.display()));
            let config_dir = easytier.get_config_dir();

            #[cfg(target_os = "macos")]
            match crate::easytier::EasyTierProcess::start_daemon(
                &binary, &config_dir, ipc::EASYTIER_DAEMON_RPC_PORT,
            ).await {
                Ok(_) => {
                    crate::log_info!("[Daemon] easytier-core 守护进程就绪");
                }
                Err(e) => {
                    crate::log_error!(format!("[Daemon] easytier-core 守护进程启动失败: {}", e));
                    let log_path = config_dir.join("easytier-daemon.log");
                    if let Ok(content) = std::fs::read_to_string(&log_path) {
                        crate::log_error!(format!("[Daemon] easytier-daemon.log 末尾:\n{}",
                            if content.len() > 2000 { &content[content.len()-2000..] } else { &content }));
                    }
                }
            }

            #[cfg(not(target_os = "macos"))]
            match crate::easytier::EasyTierProcess::start_daemon(
                &binary, &config_dir, ipc::EASYTIER_DAEMON_RPC_PORT,
            ).await {
                Ok(_) => {
                    crate::log_info!("[Daemon] easytier-core 守护进程就绪");
                }
                Err(e) => {
                    crate::log_error!(format!("[Daemon] easytier-core 守护进程启动失败: {}", e));
                }
            }
        });

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
                    let _ = std::fs::remove_file(self.data_dir.join("daemon_state.json"));
                    let _ = std::fs::remove_file(self.data_dir.join("daemon_ready.signal"));
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
                crate::log_info!(format!("[Daemon] ConnectSpace: 接收连接请求, space_id={}", space_id));

                // 解析配置
                let network_config: crate::easytier::config::NetworkConfig = match serde_json::from_value::<crate::easytier::config::NetworkConfig>(config) {
                    Ok(c) => {
                        crate::log_info!(format!("[Daemon] ConnectSpace: 配置解析成功, network_name={}, dhcp={}, peers={}", c.network_name, c.dhcp, c.peers.len()));
                        c
                    }
                    Err(e) => {
                        crate::log_error!(format!("[Daemon] ConnectSpace: 配置解析失败: {}", e));
                        return ipc::IpcResponse::Error { message: format!("解析配置失败: {}", e) };
                    }
                };

                // 启动 easytier
                let instance_id = match uuid::Uuid::parse_str(&space_id) {
                    Ok(id) => {
                        crate::log_debug!(format!("[Daemon] ConnectSpace: space_id 解析为 Uuid: {}", id));
                        id
                    }
                    Err(_) => {
                        crate::log_error!("[Daemon] ConnectSpace: 无效的 space_id 格式");
                        return ipc::IpcResponse::Error { message: "无效的 space_id".into() };
                    }
                };

                crate::log_info!(format!("[Daemon] ConnectSpace: 调用 easytier.start_network, instance_id={}", instance_id));
                match easytier.start_network(&network_config, instance_id, None).await {
                    Ok(id) => {
                        crate::log_info!(format!("[Daemon] ConnectSpace: easytier.start_network 成功, id={}", id));
                        let mut s = status.write().await;
                        if !s.connected_spaces.contains(&space_id) {
                            s.connected_spaces.push(space_id);
                        }
                        ipc::IpcResponse::Ok { data: Some(serde_json::json!({ "instance_id": id.to_string() })) }
                    }
                    Err(e) => {
                        crate::log_error!(format!("[Daemon] ConnectSpace: easytier.start_network 失败: {}", e));
                        ipc::IpcResponse::Error { message: format!("连接空间失败: {}", e) }
                    }
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
                                crate::log_info!(format!("[Daemon] ListPeers 结果: peer 数量={}", peers.len()));
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
                match source_path {
                    Some(ref path) => {
                        crate::log_info!(format!("[Daemon] 升级版本: {}, 本地路径: {}", version, path));
                        let source = Some(crate::easytier::BinarySource::LocalBinary(std::path::PathBuf::from(path)));
                        match easytier.upgrade(&version, source).await {
                            Ok(()) => ipc::IpcResponse::Ok { data: None },
                            Err(e) => ipc::IpcResponse::Error { message: format!("升级失败: {}", e) },
                        }
                    }
                    None => {
                        crate::log_info!(format!("[Daemon] 升级版本: {}, 从 GitHub 下载", version));
                        match easytier.upgrade(&version, None).await {
                            Ok(()) => ipc::IpcResponse::Ok { data: None },
                            Err(e) => ipc::IpcResponse::Error { message: format!("升级失败: {}", e) },
                        }
                    }
                }
            }
            ipc::IpcRequest::SwitchBinary => {
                crate::log_info!("[Daemon] 收到切换二进制命令");
                easytier.restart_all_instances().await;
                ipc::IpcResponse::Ok { data: None }
            }
            ipc::IpcRequest::GetLogs { level, since_seq, space_id } => {
                let level_filter = level.and_then(|l| match l.to_lowercase().as_str() {
                    "debug" => Some(crate::log::LogLevel::Debug),
                    "info" => Some(crate::log::LogLevel::Info),
                    "warning" => Some(crate::log::LogLevel::Warning),
                    "error" => Some(crate::log::LogLevel::Error),
                    _ => None,
                });
                let logs = match &space_id {
                    Some(sid) => crate::log::get_by_space(sid, level_filter),
                    None => crate::log::get_all(level_filter),
                };
                let filtered: Vec<crate::log::LogEntry> = match since_seq {
                    Some(s) => logs.into_iter().filter(|e| e.seq > s).collect(),
                    None => logs,
                };
                match serde_json::to_value(&filtered) {
                    Ok(v) => ipc::IpcResponse::Ok { data: Some(v) },
                    Err(e) => ipc::IpcResponse::Error { message: format!("序列化日志失败: {}", e) },
                }
            }
            ipc::IpcRequest::WriteLog { entries } => {
                for e in entries {
                    crate::log::log(e.level, &e.module, e.message, e.space_id);
                }
                ipc::IpcResponse::Ok { data: None }
            }
            ipc::IpcRequest::ClearDaemonLogs => {
                crate::log::clear();
                ipc::IpcResponse::Ok { data: None }
            }
            ipc::IpcRequest::CheckBinary => {
                crate::log_info!("[Daemon] 检查 EasyTier 二进制");
                match easytier.downloader.ensure_binary().await {
                    Ok(binary_path) => {
                        use tokio::process::Command;
                        let output = Command::new(&binary_path)
                            .arg("--version")
                            .output()
                            .await;
                        match output {
                            Ok(out) => {
                                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                                let result = serde_json::json!({
                                    "binary": binary_path.to_string_lossy(),
                                    "version": stdout.trim(),
                                    "stderr": stderr.trim(),
                                    "success": out.status.success(),
                                });
                                ipc::IpcResponse::Ok { data: Some(result) }
                            }
                            Err(e) => ipc::IpcResponse::Error { message: format!("执行二进制失败: {}", e) },
                        }
                    }
                    Err(e) => ipc::IpcResponse::Error { message: format!("获取二进制失败: {}", e) },
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

/// daemon 入口点（路径由 GUI 通过 CLI 传入）
pub async fn run_daemon_async(config_dir: PathBuf, data_dir: PathBuf) -> Result<(), String> {
    let daemon = Daemon::new(config_dir, data_dir)?;
    daemon.run().await
}
