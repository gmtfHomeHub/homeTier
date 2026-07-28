pub mod config;
pub mod downloader;
pub mod github;
pub mod process;

pub mod launcher {
    pub use super::launcher_internal::*;
}

use std::sync::Arc;
use dashmap::DashMap;
use uuid::Uuid;
use tokio::sync::RwLock;
use std::path::PathBuf;

pub use downloader::{EasyTierDownloader, BinarySource};
pub use process::EasyTierProcess;

use crate::types::NetworkStatus;

/// EasyTier 管理器，管理多个网络实例（Desktop 使用子进程，Mobile 使用库）
pub struct EasyTierManager {
    /// 运行中的进程: space_id → process (Desktop)
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    processes: DashMap<Uuid, EasyTierProcess>,
    /// 运行中的实例: space_id → RunningInstance (Mobile)
    #[cfg(any(target_os = "android", target_os = "ios"))]
    instances: DashMap<Uuid, launcher_internal::RunningInstance>,
    /// 二进制下载器
    pub downloader: EasyTierDownloader,
    /// 配置目录
    config_dir: PathBuf,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl EasyTierManager {
    pub fn new(config_dir: PathBuf, app_data_dir: PathBuf) -> Self {
        let downloader = EasyTierDownloader::new(&app_data_dir);

        Self { processes: DashMap::new(), downloader, config_dir }
    }

    /// 获取配置文件目录
    pub fn get_config_dir(&self) -> PathBuf {
        self.config_dir.clone()
    }

    /// 启动网络实例（Desktop: 通过 RPC 调用已运行的 easytier-core daemon）
    pub async fn start_network(
        &self,
        cfg: &config::NetworkConfig,
        instance_id: Uuid,
        initial_config: Option<String>,
    ) -> Result<Uuid, String> {
        crate::log_info!(format!("EasyTierManager.start_network: 开始, network_name={}, id={}", cfg.network_name, instance_id));

        // 确保二进制存在（用于验证，实际启动由 daemon 完成）
        crate::log_debug!("EasyTierManager.start_network: 确保二进制存在");
        let _ = self.downloader.ensure_binary().await.map_err(|e| {
            crate::log_error!(format!("EasyTierManager.start_network: 确保二进制失败: {}", e));
            e
        })?;

        // 生成 TOML 配置文件（写入 daemon 共享的 config_dir，供 Reload 使用）
        crate::log_debug!("EasyTierManager.start_network: 生成配置文件");
        let _ = self.generate_config(cfg, &instance_id, initial_config.as_deref())?;

        // 构建 protobuf 配置
        let proto_cfg = self.build_proto_config(cfg, &instance_id);

        // 通过 RPC 调用 easytier-core-daemon 启动网络实例
        crate::log_info!("EasyTierManager.start_network: 调用 RPC run_network_instance");
        self.rpc_run_network_instance(&instance_id, &proto_cfg).await?;

        crate::log_info!(format!("EasyTierManager.start_network: 完成, id={}", instance_id));
        Ok(instance_id)
    }

    /// 构建 protobuf NetworkConfig
    fn build_proto_config(&self, cfg: &config::NetworkConfig, instance_id: &Uuid) -> easytier::proto::api::manage::NetworkConfig {
        use easytier::proto::api::manage::{NetworkConfig, NetworkingMethod};
        
        // 映射 networking_method
        let (networking_method, public_server_url) = match cfg.networking_method {
            0 => (Some(NetworkingMethod::PublicServer as i32), 
                  if cfg.public_server_url.is_empty() { None } else { Some(cfg.public_server_url.clone()) }),
            1 => (Some(NetworkingMethod::Manual as i32), None),
            2 => (Some(NetworkingMethod::Standalone as i32), None),
            _ => (None, None),
        };
        
        NetworkConfig {
            instance_id: Some(instance_id.to_string()),
            network_name: if cfg.network_name.is_empty() { None } else { Some(cfg.network_name.clone()) },
            network_secret: if cfg.network_secret.is_empty() { None } else { Some(cfg.network_secret.clone()) },
            dhcp: Some(cfg.dhcp),
            virtual_ipv4: cfg.ipv4.clone(),
            hostname: cfg.hostname.clone(),
            listener_urls: cfg.listeners.clone(),
            peer_urls: cfg.peers.iter().map(|p| p.uri.clone()).collect(),
            proxy_cidrs: cfg.proxy_networks.iter().map(|p| p.cidr.clone()).collect(),
            routes: cfg.routes.clone(),
            exit_nodes: cfg.exit_nodes.clone(),
            port_forwards: Vec::new(),
            dev_name: cfg.flags.get("dev_name").cloned(),
            mtu: cfg.flags.get("mtu").and_then(|v| v.parse().ok()),
            latency_first: cfg.flags.get("latency_first").map(|v| v == "true"),
            enable_kcp_proxy: cfg.flags.get("enable_kcp_proxy").map(|v| v == "true"),
            enable_quic_proxy: cfg.flags.get("enable_quic_proxy").map(|v| v == "true"),
            no_tun: cfg.flags.get("no_tun").map(|v| v == "true"),
            disable_p2p: cfg.flags.get("disable_p2p").map(|v| v == "true"),
            multi_thread: cfg.flags.get("multi_thread").map(|v| v == "true"),
            bind_device: cfg.flags.get("bind_device").map(|v| v == "true"),
            disable_encryption: None,
            disable_ipv6: None,
            encryption_algorithm: cfg.flags.get("encryption_algorithm").cloned(),
            networking_method,
            public_server_url,
            ..Default::default()
        }
    }

    /// 通过 RPC 调用 easytier-core-daemon 启动网络实例
    async fn rpc_run_network_instance(&self, instance_id: &Uuid, config: &easytier::proto::api::manage::NetworkConfig) -> Result<(), String> {
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::tunnel::tcp::TcpTunnelConnector;
        use easytier::proto::api::manage::{WebClientServiceClientFactory, RunNetworkInstanceRequest};

        let rpc_port = crate::daemon::ipc::EASYTIER_DAEMON_RPC_PORT;
        let addr = format!("tcp://127.0.0.1:{}", rpc_port);
        let url: url::Url = addr.parse().map_err(|e| format!("解析 RPC 地址失败: {}", e))?;

        let connector = TcpTunnelConnector::new(url);
        let mut client = StandAloneClient::new(connector);

        let ctrl = BaseController::default();
        let manage_service = client.scoped_client::<WebClientServiceClientFactory<BaseController>>("".to_string()).await
            .map_err(|e| format!("连接 easytier-core RPC 失败: {}", e))?;

        let req = RunNetworkInstanceRequest {
            inst_id: Some((*instance_id).into()),
            config: Some(config.clone()),
            overwrite: true,
            source: easytier::proto::api::manage::ConfigSource::User as i32,
        };

        manage_service.run_network_instance(ctrl, req).await
            .map_err(|e| format!("RPC run_network_instance 失败: {}", e))?;

        Ok(())
    }

    /// Phase 1-1: 等待进程 RPC 端口就绪
    async fn wait_for_rpc_ready(&self, _instance_id: &Uuid, rpc_port: u16, timeout: std::time::Duration) -> Result<(), String> {
        let start = std::time::Instant::now();
        let mut last_err = String::new();
        while start.elapsed() < timeout {
            match tokio::net::TcpStream::connect(format!("127.0.0.1:{}", rpc_port)).await {
                Ok(_) => {
                    crate::log_info!(format!("EasyTierManager.wait_for_rpc_ready: RPC 端口就绪, port={}, elapsed={:?}", rpc_port, start.elapsed()));
                    return Ok(());
                }
                Err(e) => {
                    last_err = e.to_string();
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                }
            }
        }
        let msg = format!("RPC 端口超时未就绪, port={}, timeout={:?}, 最后错误: {}", rpc_port, timeout, last_err);
        crate::log_warn!(format!("EasyTierManager.wait_for_rpc_ready: {}", msg));
        Err(msg)
    }

    /// 为实例分配唯一的 RPC 端口
    fn allocate_rpc_port(&self, instance_id: &Uuid) -> u16 {
        // 基于 instance_id 哈希生成端口号（范围 15900-15999）
        let hash = instance_id.as_fields().0 as u16;
        let base = 15900u16;
        let offset = hash % 100;
        base + offset
    }

    /// 停止网络实例（Desktop: 通过 RPC 调用）
    pub async fn stop_network(&self, instance_id: &Uuid) -> Result<Option<String>, String> {
        crate::log_info!(format!("EasyTierManager: 停止网络实例, id={}", instance_id));

        let config = self.read_config(instance_id);

        // 通过 RPC 删除网络实例（不杀 easytier-core 进程）
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::tunnel::tcp::TcpTunnelConnector;
        use easytier::proto::api::manage::{WebClientServiceClientFactory, DeleteNetworkInstanceRequest};

        let rpc_port = crate::daemon::ipc::EASYTIER_DAEMON_RPC_PORT;
        let addr = format!("tcp://127.0.0.1:{}", rpc_port);
        let url: url::Url = addr.parse().map_err(|e| format!("解析 RPC 地址失败: {}", e))?;

        let connector = TcpTunnelConnector::new(url);
        let mut client = StandAloneClient::new(connector);

        let ctrl = BaseController::default();
        let manage_service = client.scoped_client::<WebClientServiceClientFactory<BaseController>>("".to_string()).await
            .map_err(|e| format!("连接 easytier-core RPC 失败: {}", e))?;

        let req = DeleteNetworkInstanceRequest {
            inst_ids: vec![(*instance_id).into()],
        };

        match manage_service.delete_network_instance(ctrl, req).await {
            Ok(_) => {
                crate::log_info!(format!("EasyTierManager: 网络实例已停止, id={}", instance_id));
            }
            Err(e) => {
                crate::log_warn!(format!("EasyTierManager: delete_network_instance failed: {}, may already be stopped", e));
            }
        }

        Ok(config)
    }

    /// 获取网络状态
    pub async fn get_status(&self, instance_id: &Uuid) -> Result<NetworkStatus, String> {
        let rpc_port = self.get_instance_rpc_port(instance_id);
        let is_running = rpc_port.map(|_| self.is_running(instance_id)).unwrap_or(false);
        let virtual_ip = if is_running {
            self.query_virtual_ip(instance_id).await
        } else {
            None
        };

        Ok(NetworkStatus {
            space_id: *instance_id,
            status: if is_running { "connected".into() } else { "disconnected".into() },
            virtual_ip,
            latency_ms: None,
            connected_peers: if is_running { self.query_peer_count(instance_id).await } else { 0 },
        })
    }

    /// 生成 TOML 配置文件
    fn generate_config(
        &self,
        cfg: &config::NetworkConfig,
        instance_id: &Uuid,
        initial_config: Option<&str>,
    ) -> Result<PathBuf, String> {
        use easytier::common::config::ConfigLoader;
        let easytier_cfg = cfg.to_easytier_config()?;

        // 应用运行时配置（flags 等）
        if let Some(config_str) = initial_config {
            self.apply_runtime_config(&easytier_cfg, config_str);
        }

        let config_content = easytier_cfg.dump();
        let config_file_name = format!("{}.toml", instance_id);
        let config_path = self.config_dir.join(&config_file_name);

        std::fs::create_dir_all(&self.config_dir)
            .map_err(|e| format!("创建配置目录失败: {}", e))?;
        std::fs::write(&config_path, &config_content)
            .map_err(|e| format!("写入配置文件失败: {}", e))?;

        crate::log_info!(format!("EasyTierManager: 配置文件已生成, 内容:\n{}", config_content));
        Ok(config_path)
    }

    /// 应用运行时配置
    fn apply_runtime_config(&self, cfg: &easytier::common::config::TomlConfigLoader, config_str: &str) {
        use easytier::common::config::ConfigLoader;
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(config_str) {
            // 应用 flags
            if let Some(flags) = json_val.get("flags").and_then(|f| f.as_object()) {
                let mut easy_flags = cfg.get_flags();
                for (key, val) in flags {
                    let s = match val {
                        serde_json::Value::String(v) => v.clone(),
                        serde_json::Value::Bool(v) => v.to_string(),
                        serde_json::Value::Number(v) => v.to_string(),
                        _ => continue,
                    };
                    match key.as_str() {
                        "mtu" => if let Ok(v) = s.parse::<u32>() { easy_flags.mtu = v; },
                        "latency_first" => easy_flags.latency_first = s == "true",
                        "enable_kcp_proxy" => easy_flags.enable_kcp_proxy = s == "true",
                        "enable_quic_proxy" => easy_flags.enable_quic_proxy = s == "true",
                        "encryption_algorithm" => easy_flags.encryption_algorithm = s,
                        "no_tun" => easy_flags.no_tun = s == "true",
                        "disable_p2p" => easy_flags.disable_p2p = s == "true",
                        "multi_thread" => easy_flags.multi_thread = s == "true",
                        "bind_device" => easy_flags.bind_device = s == "true",
                        "default_protocol" => easy_flags.default_protocol = s,
                        "dev_name" => easy_flags.dev_name = s,
                        "enable_encryption" => easy_flags.enable_encryption = s == "true",
                        "enable_ipv6" => easy_flags.enable_ipv6 = s == "true",
                        _ => {},
                    }
                }
                cfg.set_flags(easy_flags);
            }
            // 应用其他配置
            if let Some(hostname) = json_val.get("hostname").and_then(|v| v.as_str()) {
                if !hostname.is_empty() { cfg.set_hostname(Some(hostname.to_string())); }
            }
            if let Some(ipv4) = json_val.get("ipv4").and_then(|v| v.as_str()) {
                if !ipv4.is_empty() {
                    if let Ok(inet) = format!("{}/24", ipv4).parse::<cidr::Ipv4Inet>() {
                        cfg.set_ipv4(Some(inet));
                    }
                }
            }
            if let Some(dhcp) = json_val.get("dhcp").and_then(|v| v.as_bool()) {
                cfg.set_dhcp(dhcp);
            }
            if let Some(ni) = json_val.get("network_identity") {
                let nn = ni.get("network_name").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                let ns = ni.get("network_secret").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                if nn.is_some() || ns.is_some() {
                    cfg.set_network_identity(easytier::common::config::NetworkIdentity::new(
                        nn.unwrap_or("").to_string(),
                        ns.unwrap_or("").to_string(),
                    ));
                }
            }
            if let Some(peers) = json_val.get("peers").and_then(|v| v.as_array()) {
                let easy_peers: Vec<easytier::common::config::PeerConfig> = peers
                    .iter()
                    .filter_map(|p| {
                        let uri = p.get("uri").and_then(|u| u.as_str())?;
                        if uri.is_empty() { return None; }
                        let url = uri.parse::<url::Url>().ok()?;
                        let pubkey = p.get("peer_public_key").and_then(|k| k.as_str()).map(|s| s.to_string());
                        Some(easytier::common::config::PeerConfig { uri: url, peer_public_key: pubkey })
                    })
                    .collect();
                if !easy_peers.is_empty() { cfg.set_peers(easy_peers); }
            }
        }
    }

    /// 读取配置文件内容
    fn read_config(&self, instance_id: &Uuid) -> Option<String> {
        let path = self.config_dir.join(format!("{}.toml", instance_id));
        std::fs::read_to_string(path).ok()
    }

    /// 通过 RPC 查询虚拟 IP
    async fn query_virtual_ip(&self, instance_id: &Uuid) -> Option<String> {
        let rpc_port = self.get_instance_rpc_port(instance_id)?;
        self.query_rpc_status(instance_id, rpc_port).await
            .map(|s| s.virtual_ip)
            .flatten()
    }

    /// 通过 RPC 查询 peer 数量
    async fn query_peer_count(&self, instance_id: &Uuid) -> u32 {
        let rpc_port = match self.get_instance_rpc_port(instance_id) {
            Some(p) => p,
            None => return 0,
        };
        self.query_rpc_status(instance_id, rpc_port).await
            .map(|s| s.connected_peers)
            .unwrap_or(0)
    }

    /// 通过 RPC 查询完整的运行时状态
    async fn query_rpc_status(&self, instance_id: &Uuid, rpc_port: u16) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::tunnel::tcp::TcpTunnelConnector;
        use easytier::proto::api::manage::WebClientServiceClientFactory;

        let addr = format!("tcp://127.0.0.1:{}", rpc_port);
        let url: url::Url = addr.parse().ok()?;

        let connector = TcpTunnelConnector::new(url);
        let mut client = StandAloneClient::new(connector);

        // 通过 collect_network_info 获取实例状态（兼容 daemon 模式）
        let ctrl = BaseController::default();
        let web_service = client.scoped_client::<WebClientServiceClientFactory<BaseController>>("".to_string()).await.ok()?;

        let proto_uuid: easytier::proto::common::Uuid = (*instance_id).into();
        let inst_id_str = instance_id.to_string();
        let collect_req = easytier::proto::api::manage::CollectNetworkInfoRequest {
            inst_ids: vec![proto_uuid],
        };
        match web_service.collect_network_info(ctrl, collect_req).await {
            Ok(resp) => {
                let running_info = resp.info.as_ref()
                    .and_then(|m| m.map.get(&inst_id_str))?;

                let mut virtual_ip = None;
                let mut connected_peers = 0u32;

                if let Some(ref my_node) = running_info.my_node_info {
                    if let Some(ref ipv4_inet) = my_node.virtual_ipv4 {
                        if let Some(ref addr) = ipv4_inet.address {
                            virtual_ip = Some(addr.to_string());
                        }
                    }
                }
                connected_peers = running_info.peers.len() as u32;

                // 计算平均延迟（从 PeerConnInfo.stats 聚合）
                let mut total_latency = 0.0f64;
                let mut latency_count = 0u32;
                let mut total_rx_bytes = 0u64;
                let mut total_tx_bytes = 0u64;

                for peer in &running_info.peers {
                    for conn in &peer.conns {
                        if let Some(stats) = &conn.stats {
                            total_latency += stats.latency_us as f64;
                            total_rx_bytes += stats.rx_bytes;
                            total_tx_bytes += stats.tx_bytes;
                            latency_count += 1;
                        }
                    }
                }

                let avg_latency_ms = if latency_count > 0 { total_latency / latency_count as f64 / 1000.0 } else { 0.0 };

                Some(crate::daemon::ipc::SpaceRuntimeStatus {
                    space_id: instance_id.to_string(),
                    is_running: true,
                    virtual_ip,
                    connected_peers,
                    rx_bytes: total_rx_bytes,
                    tx_bytes: total_tx_bytes,
                    avg_latency_ms,
                })
            }
            Err(e) => {
                crate::log_warn!(format!("EasyTierManager: RPC 查询失败, port={}, error={}", rpc_port, e));
                None
            }
        }
    }

    /// 获取详细的网络统计信息
    pub async fn get_network_stats(&self, instance_id: &Uuid) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        let rpc_port = self.get_instance_rpc_port(instance_id)?;
        self.query_rpc_status(instance_id, rpc_port).await
    }

    /// 获取实例的 RPC 端口（共享 daemon 端口）
    fn get_instance_rpc_port(&self, instance_id: &Uuid) -> Option<u16> {
        Some(crate::daemon::ipc::EASYTIER_DAEMON_RPC_PORT)
    }

    /// 获取连接的 peer 数量
    pub fn get_connected_peers(&self, instance_id: &Uuid) -> Option<u32> {
        self.get_instance_rpc_port(instance_id).map(|_| 0) // 同步方法无法查询 RPC，返回 0
    }

    /// 获取虚拟 IP
    pub fn get_virtual_ip(&self, instance_id: &Uuid) -> Option<String> {
        self.get_instance_rpc_port(instance_id).and_then(|_| None) // 同步方法无法查询 RPC
    }

    /// 获取 peer 列表（通过 RPC 查询）
    pub async fn get_peers(&self, instance_id: &Uuid) -> Result<Vec<crate::easytier::launcher::PeerInfo>, String> {
        let rpc_port = self.get_instance_rpc_port(instance_id)
            .ok_or_else(|| "未找到 RPC 端口".to_string())?;

        self.query_peer_list(instance_id, rpc_port).await
            .ok_or_else(|| "查询 peer 列表失败".to_string())
    }

    /// 通过 RPC 查询 peer 列表（含重试）
    async fn query_peer_list(&self, instance_id: &Uuid, rpc_port: u16) -> Option<Vec<crate::easytier::launcher::PeerInfo>> {
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::proto::api::instance::PeerManageRpcClientFactory;
        use easytier::tunnel::tcp::TcpTunnelConnector;

        let addr = format!("tcp://127.0.0.1:{}", rpc_port);
        let url: url::Url = addr.parse().ok()?;

        let max_retries = 3;
        for attempt in 1..=max_retries {
            let connector = TcpTunnelConnector::new(url.clone());
            let mut client = StandAloneClient::new(connector);

            let ctrl = BaseController::default();
            let peer_service = client.scoped_client::<PeerManageRpcClientFactory<BaseController>>("".to_string()).await;

            if let Ok(peer_service) = peer_service {
                let list_req = easytier::proto::api::instance::ListPeerRequest {
                    instance: Some(easytier::proto::api::instance::InstanceIdentifier {
                        selector: Some(easytier::proto::api::instance::instance_identifier::Selector::Id(instance_id.to_string())),
                    }),
                    ..Default::default()
                };
                let result = peer_service.list_peer(ctrl, list_req).await;
                match result {
                    Ok(resp) => {
                        crate::log_debug!(format!("[D20260728-V3] query_peer_list response: my_info={:?}, peer_count={}", resp.my_info, resp.peer_infos.len()));
                        let mut peer_infos = Vec::new();

                        for peer in resp.peer_infos {
                            let mut total_rx = 0u64;
                            let mut total_tx = 0u64;
                            let mut total_latency_us = 0u64;
                            let mut total_loss_rate = 0.0f32;
                            let mut conn_count = 0u32;
                            let mut tunnel_proto = None;

                            for conn in &peer.conns {
                                if let Some(stats) = &conn.stats {
                                    total_rx += stats.rx_bytes;
                                    total_tx += stats.tx_bytes;
                                    total_latency_us += stats.latency_us;
                                    total_loss_rate += conn.loss_rate;
                                    conn_count += 1;
                                }
                                if let Some(ref tunnel) = conn.tunnel {
                                    tunnel_proto = Some(tunnel.tunnel_type.clone());
                                }
                            }

                            let avg_latency_ms = if conn_count > 0 { Some(total_latency_us as f64 / conn_count as f64 / 1000.0) } else { None };
                            let avg_loss_rate = if conn_count > 0 { Some(total_loss_rate / conn_count as f32) } else { None };
                            let any_connected = peer.conns.iter().any(|c| !c.is_closed);

                            peer_infos.push(crate::easytier::launcher::PeerInfo {
                                peer_id: peer.peer_id,
                                virtual_ip: None,
                                hostname: None,
                                latency_ms: avg_latency_ms,
                                loss_rate: avg_loss_rate.map(|f| f as f64),
                                rx_bytes: Some(total_rx),
                                tx_bytes: Some(total_tx),
                                connected: any_connected,
                                is_local: false,
                                version: None,
                                tunnel_proto,
                                nat_type: None,
                            });
                        }

                        if attempt > 1 {
                            crate::log_info!(format!("EasyTierManager: RPC 查询成功 (第{}次重试), peer数量={}", attempt, peer_infos.len()));
                        }
                        return Some(peer_infos);
                    }
                    Err(e) => {
                        if attempt < max_retries {
                            crate::log_warn!(format!("EasyTierManager: RPC 查询失败 (第{}/{}次), port={}, error={}, 即将重试", attempt, max_retries, rpc_port, e));
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        } else {
                            crate::log_warn!(format!("EasyTierManager: RPC 查询失败 (已重试{}次), port={}, error={}", max_retries, rpc_port, e));
                            return None;
                        }
                    }
                }
            } else {
                if attempt < max_retries {
                    crate::log_warn!(format!("EasyTierManager: RPC 连接失败 (第{}/{}次), port={}, 即将重试", attempt, max_retries, rpc_port));
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                } else {
                    crate::log_warn!(format!("EasyTierManager: RPC 连接失败 (已重试{}次), port={}", max_retries, rpc_port));
                    return None;
                }
            }
        }

        None
    }

    /// 获取空间运行时状态（通过 RPC 查询）
    pub async fn get_space_status(&self, instance_id: &Uuid) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        let rpc_port = self.get_instance_rpc_port(instance_id)?;
        self.query_rpc_status(instance_id, rpc_port).await
    }

    /// 运行时修改配置（重启子进程应用新配置）
    pub async fn patch_config(
        &self,
        instance_id: &Uuid,
        patch: &serde_json::Value,
    ) -> Result<(), String> {
        crate::log_info!(format!("EasyTierManager: patch_config, id={}", instance_id));

        // 读取现有配置文件
        let config_path = self.config_dir.join(format!("{}.toml", instance_id));
        if !config_path.exists() {
            return Err(format!("配置文件不存在: {}", config_path.display()));
        }

        // 读取现有 NetworkConfig（从 TOML 反序列化）
        let toml_content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("读取配置文件失败: {}", e))?;

        // 解析 patch 中的字段并应用
        // patch 格式: { "network_name": "...", "network_secret": "...", "flags": {...}, ... }
        let mut network_config = self.read_network_config(instance_id)?;

        if let Some(name) = patch.get("network_name").and_then(|v| v.as_str()) {
            network_config.network_name = name.to_string();
        }
        if let Some(secret) = patch.get("network_secret").and_then(|v| v.as_str()) {
            network_config.network_secret = secret.to_string();
        }
        if let Some(dhcp) = patch.get("dhcp").and_then(|v| v.as_bool()) {
            network_config.dhcp = dhcp;
        }
        if let Some(ipv4) = patch.get("ipv4").and_then(|v| v.as_str()) {
            network_config.ipv4 = Some(ipv4.to_string());
        }
        if let Some(peers) = patch.get("peers").and_then(|v| v.as_array()) {
            network_config.peers = peers.iter().filter_map(|p| {
                let uri = p.get("uri")?.as_str()?.to_string();
                let peer_public_key = p.get("peer_public_key").and_then(|k| k.as_str()).map(|s| s.to_string());
                Some(config::PeerConfig { uri, peer_public_key })
            }).collect();
        }
        if let Some(flags) = patch.get("flags").and_then(|v| v.as_object()) {
            for (key, val) in flags {
                if let Some(s) = val.as_str() {
                    network_config.flags.insert(key.clone(), s.to_string());
                }
            }
        }

        // 重新生成配置文件
        let _ = self.generate_config(&network_config, instance_id, None)?;

        // 通过 RPC 重新启动网络实例（覆盖已有）
        let proto_cfg = self.build_proto_config(&network_config, instance_id);
        self.rpc_run_network_instance(instance_id, &proto_cfg).await?;
        crate::log_info!(format!("EasyTierManager: 配置已更新, id={}", instance_id));

        Ok(())
    }

    /// 读取空间的 NetworkConfig
    fn read_network_config(&self, instance_id: &Uuid) -> Result<config::NetworkConfig, String> {
        let config_path = self.config_dir.join(format!("{}.toml", instance_id));
        if !config_path.exists() {
            return Err(format!("配置文件不存在: {}", config_path.display()));
        }

        let toml_content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("读取配置文件失败: {}", e))?;

        // 从 TOML 反序列化为 NetworkConfig
        // 由于 TOML 格式与 NetworkConfig 不完全兼容，这里使用简单解析
        let mut network_config = config::NetworkConfig::default();

        // 解析 network_identity
        if let Some(ni) = toml_content.lines().find(|l| l.starts_with("[network_identity]")) {
            // 简单解析 TOML section
            for line in toml_content.lines().skip_while(|l| !l.starts_with("[network_identity]")).take_while(|l| !l.starts_with('[')) {
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"');
                    match key {
                        "network_name" => network_config.network_name = value.to_string(),
                        "network_secret" => network_config.network_secret = value.to_string(),
                        _ => {}
                    }
                }
            }
        }

        Ok(network_config)
    }

    /// 检查网络是否正在运行
    pub fn is_running(&self, instance_id: &Uuid) -> bool {
        // 通过 RPC 查询实例列表来检查
        self.read_config_path(instance_id).is_some()
    }

    /// 获取所有运行的实例 ID
    pub fn list_running(&self) -> Vec<Uuid> {
        self.list_saved().iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect()
    }

    /// 获取所有已保存的 space 配置列表
    pub fn list_saved(&self) -> Vec<String> {
        let mut spaces = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.config_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".toml") {
                        if let Some(space_id) = name.strip_suffix(".toml") {
                            spaces.push(space_id.to_string());
                        }
                    }
                }
            }
        }
        spaces
    }

    /// 获取当前版本
    pub async fn get_version(&self) -> Result<String, String> {
        self.downloader.current_version()
            .ok_or_else(|| "EasyTier 未安装".into())
    }

    /// 升级版本（source 为 None 时自动从 GitHub 下载）
    pub async fn upgrade(&self, version: &str, source: Option<BinarySource>) -> Result<(), String> {
        if let Some(source) = source {
            self.downloader.install(version, source).await?;
        } else {
            self.downloader.download_from_github(version).await?;
        }
        self.restart_all_instances().await;
        Ok(())
    }

    /// 重启所有运行中的实例（升级后调用）
    pub(crate) async fn restart_all_instances(&self) {
        let running = self.list_running();
        for space_id in running {
            crate::log_info!(format!("EasyTierManager: 重启实例以应用新版本, id={}", space_id));
            if let Some(_config_path) = self.read_config_path(&space_id) {
                let cfg = match self.read_network_config(&space_id) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let proto_cfg = self.build_proto_config(&cfg, &space_id);
                let _ = self.rpc_run_network_instance(&space_id, &proto_cfg).await;
            }
        }
    }

    /// 获取配置路径
    fn read_config_path(&self, instance_id: &Uuid) -> Option<PathBuf> {
        let path = self.config_dir.join(format!("{}.toml", instance_id));
        if path.exists() { Some(path) } else { None }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
impl EasyTierManager {
    pub fn new(config_dir: PathBuf, app_data_dir: PathBuf) -> Self {
        let downloader = EasyTierDownloader::new(&app_data_dir);

        Self { instances: DashMap::new(), downloader, config_dir }
    }

    /// 启动网络实例（Mobile: 库方式）
    pub async fn start_network(
        &self,
        cfg: &config::NetworkConfig,
        instance_id: Uuid,
        initial_config: Option<String>,
    ) -> Result<Uuid, String> {
        crate::log_info!(format!("EasyTierManager: 启动网络实例 (Mobile), network_name={}, id={}", cfg.network_name, instance_id));

        // 停止现有实例
        if self.instances.contains_key(&instance_id) {
            self.stop_network(&instance_id).await?;
        }

        // 使用库方式启动
        let running = launcher_internal::start_easytier(cfg, instance_id, &self.config_dir, initial_config).await?;
        self.instances.insert(instance_id, running);

        crate::log_info!(format!("EasyTierManager: 网络实例已启动 (Mobile), id={}", instance_id));
        Ok(instance_id)
    }

    /// 停止网络实例
    pub async fn stop_network(&self, instance_id: &Uuid) -> Result<Option<String>, String> {
        crate::log_info!(format!("EasyTierManager: 停止网络实例 (Mobile), id={}", instance_id));
        if let Some((_, mut instance)) = self.instances.remove(instance_id) {
            let config = instance.stop().await?;
            crate::log_info!(format!("EasyTierManager: 网络实例已停止 (Mobile), id={}", instance_id));
            Ok(config)
        } else {
            crate::log_warn!(format!("EasyTierManager: 实例未找到 (Mobile), id={}", instance_id));
            Ok(None)
        }
    }

    /// 获取网络状态
    pub async fn get_status(&self, instance_id: &Uuid) -> Result<NetworkStatus, String> {
        let instance = self.instances.get(instance_id)
            .ok_or_else(|| {
                crate::log_warn!(format!("EasyTierManager: 获取状态失败, 实例未找到 (Mobile), id={}", instance_id));
                "Instance not found".to_string()
            })?;
        instance.get_status().await
    }

    /// 获取连接的 peer 数量
    pub fn get_connected_peers(&self, instance_id: &Uuid) -> Option<u32> {
        self.instances.get(instance_id).and_then(|inst| inst.connected_peers())
    }

    /// 获取 peer 列表
    pub async fn get_peers(&self, instance_id: &Uuid) -> Result<Vec<launcher_internal::PeerInfo>, String> {
        let instance = self.instances.get(instance_id)
            .ok_or_else(|| "Instance not found".to_string())?;
        Ok(instance.get_peers().await)
    }

    /// 获取虚拟 IP
    pub fn get_virtual_ip(&self, instance_id: &Uuid) -> Option<String> {
        self.instances.get(instance_id).and_then(|inst| inst.virtual_ip())
    }

    /// 检查网络是否正在运行
    pub fn is_running(&self, instance_id: &Uuid) -> bool {
        self.instances.contains_key(instance_id)
    }

    /// 获取所有运行的实例 ID
    pub fn list_running(&self) -> Vec<Uuid> {
        self.instances.iter().map(|e| *e.key()).collect()
    }

    /// 获取当前版本（Mobile 不使用二进制管理）
    pub async fn get_version(&self) -> Result<String, String> {
        Ok(env!("CARGO_PKG_VERSION").into())
    }

    /// 升级版本（Mobile 不支持）
    pub async fn upgrade(&self, _version: &str, _source: BinarySource) -> Result<(), String> {
        Err("Mobile 不支持版本升级".into())
    }
}

/// 运行中的实例信息（用于前端查询）
#[derive(Debug, Clone, serde::Serialize)]
pub struct RunningInstanceInfo {
    pub space_id: String,
    pub is_running: bool,
    pub pid: Option<u32>,
}

mod launcher_internal {
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use std::path::PathBuf;
    use uuid::Uuid;
    use crate::types::NetworkStatus;
    use super::config;

    /// 序列化后返回前端的 Peer 信息
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct PeerInfo {
        pub peer_id: u32,
        pub virtual_ip: Option<String>,
        pub hostname: Option<String>,
        pub latency_ms: Option<f64>,
        pub loss_rate: Option<f64>,
        pub rx_bytes: Option<u64>,
        pub tx_bytes: Option<u64>,
        pub connected: bool,
        pub is_local: bool,
        pub version: Option<String>,
        pub tunnel_proto: Option<String>,
        pub nat_type: Option<String>,
    }

    /// 运行中的实例句柄
    pub struct RunningInstance {
        pub instance_id: Uuid,
        pub network_name: String,
        pub config_path: Option<PathBuf>,
        config_content: Arc<RwLock<Option<String>>>,
        status: Arc<RwLock<InstanceStatus>>,
        instance: Option<easytier::launcher::NetworkInstance>,
    }

    struct InstanceStatus {
        virtual_ip: Option<String>,
        connected_peers: u32,
        is_running: bool,
        rx_bytes: u64,
        tx_bytes: u64,
        avg_latency_ms: f64,
        peers: Vec<PeerInfo>,
    }

    /// 启动 EasyTier 网络实例（库方式）
    pub async fn start_easytier(
        cfg: &config::NetworkConfig,
        instance_id: Uuid,
        config_dir: &PathBuf,
        initial_config: Option<String>,
    ) -> Result<RunningInstance, String> {
        use easytier::common::config::ConfigLoader;
        let network_name = cfg.network_name.clone();
        crate::log_info!(format!("start_easytier: 开始启动, network_name={}, instance_id={}", network_name, instance_id));

        let easytier_cfg = cfg.to_easytier_config()?;
        crate::log_info!(format!("start_easytier: 基本配置已加载, network_name={}, dhcp={}", network_name, cfg.dhcp), &instance_id.to_string());

        if let Some(ref config_str) = initial_config {
            crate::log_info!("start_easytier: 应用空间级配置", &instance_id.to_string());
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(config_str) {
                if let Some(flags) = json_val.get("flags").and_then(|f| f.as_object()) {
                    let mut easy_flags = easytier_cfg.get_flags();
                    for (key, val) in flags {
                        let s = match val {
                            serde_json::Value::String(v) => v.clone(),
                            serde_json::Value::Bool(v) => v.to_string(),
                            serde_json::Value::Number(v) => v.to_string(),
                            _ => continue,
                        };
                        match key.as_str() {
                            "mtu" => if let Ok(v) = s.parse::<u32>() { easy_flags.mtu = v; },
                            "latency_first" => easy_flags.latency_first = s == "true",
                            "enable_kcp_proxy" => easy_flags.enable_kcp_proxy = s == "true",
                            "enable_quic_proxy" => easy_flags.enable_quic_proxy = s == "true",
                            "encryption_algorithm" => easy_flags.encryption_algorithm = s,
                            "no_tun" => easy_flags.no_tun = s == "true",
                            "disable_p2p" => easy_flags.disable_p2p = s == "true",
                            "multi_thread" => easy_flags.multi_thread = s == "true",
                            "bind_device" => easy_flags.bind_device = s == "true",
                            "default_protocol" => easy_flags.default_protocol = s,
                            "dev_name" => easy_flags.dev_name = s,
                            "enable_encryption" => easy_flags.enable_encryption = s == "true",
                            "enable_ipv6" => easy_flags.enable_ipv6 = s == "true",
                            _ => {},
                        }
                    }
                    easytier_cfg.set_flags(easy_flags);
                }
                if let Some(hostname) = json_val.get("hostname").and_then(|v| v.as_str()) {
                    if !hostname.is_empty() { easytier_cfg.set_hostname(Some(hostname.to_string())); }
                }
                if let Some(ipv4) = json_val.get("ipv4").and_then(|v| v.as_str()) {
                    if !ipv4.is_empty() {
                        if let Ok(inet) = format!("{}/24", ipv4).parse::<cidr::Ipv4Inet>() {
                            easytier_cfg.set_ipv4(Some(inet));
                        }
                    }
                }
                if let Some(dhcp) = json_val.get("dhcp").and_then(|v| v.as_bool()) {
                    easytier_cfg.set_dhcp(dhcp);
                }
                if let Some(ni) = json_val.get("network_identity") {
                    let nn = ni.get("network_name").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                    let ns = ni.get("network_secret").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                    if nn.is_some() || ns.is_some() {
                        easytier_cfg.set_network_identity(easytier::common::config::NetworkIdentity::new(
                            nn.unwrap_or("").to_string(),
                            ns.unwrap_or("").to_string(),
                        ));
                    }
                }
                if let Some(peers) = json_val.get("peers").and_then(|v| v.as_array()) {
                    let easy_peers: Vec<easytier::common::config::PeerConfig> = peers
                        .iter()
                        .filter_map(|p| {
                            let uri = p.get("uri").and_then(|u| u.as_str())?;
                            if uri.is_empty() { return None; }
                            let url = uri.parse::<url::Url>().ok()?;
                            let pubkey = p.get("peer_public_key").and_then(|k| k.as_str()).map(|s| s.to_string());
                            Some(easytier::common::config::PeerConfig { uri: url, peer_public_key: pubkey })
                        })
                        .collect();
                    if !easy_peers.is_empty() { easytier_cfg.set_peers(easy_peers); }
                }
            }
        }

        let config_content = easytier_cfg.dump();
        let config_file_name = format!("{}.toml", network_name.replace('/', "_"));
        let config_path = config_dir.join(&config_file_name);
        std::fs::create_dir_all(config_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
        std::fs::write(&config_path, &config_content).map_err(|e| format!("写入配置文件失败: {}", e))?;

        let config_content_ref = Arc::new(RwLock::new(Some(config_content)));

        let mut instance = easytier::launcher::NetworkInstance::new(
            easytier_cfg,
            easytier::common::config::ConfigFileControl::new(
                Some(config_path.clone()),
                easytier::common::config::ConfigFilePermission::from(0u8),
            ),
        );

        instance.start().map_err(|e| {
            let err_msg = format!("EasyTier 启动失败: {:?}", e);
            crate::log_error!(&err_msg);
            err_msg
        })?;

        let api_service = {
            let mut retries = 0;
            const MAX_RETRIES: u32 = 50;
            loop {
                let svc = instance.get_api_service();
                if svc.is_some() { break svc; }
                retries += 1;
                if retries >= MAX_RETRIES {
                    let msg = format!("EasyTier RPC 服务启动超时 ({}s)", MAX_RETRIES * 200 / 1000);
                    crate::log_error!(&msg, &instance_id.to_string());
                    return Err(msg);
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        };

        let status = Arc::new(RwLock::new(InstanceStatus {
            virtual_ip: None,
            connected_peers: 0,
            is_running: true,
            rx_bytes: 0,
            tx_bytes: 0,
            avg_latency_ms: 0.0,
            peers: Vec::new(),
        }));

        let status_poll = status.clone();
        let stop_notifier = instance.get_stop_notifier();
        tokio::spawn(async move {
            poll_instance_status(status_poll, api_service).await;
        });

        let status_stop = status.clone();
        let id_str = instance_id.to_string();
        tokio::spawn(async move {
            if let Some(notifier) = stop_notifier {
                notifier.notified().await;
                let mut s = status_stop.write().await;
                s.is_running = false;
                crate::log_info!("EasyTier 实例已停止（通过停止通知器）", &id_str);
            }
        });

        Ok(RunningInstance {
            instance_id,
            network_name,
            config_path: Some(config_path),
            config_content: config_content_ref,
            status,
            instance: Some(instance),
        })
    }

    async fn poll_instance_status(
        status: Arc<RwLock<InstanceStatus>>,
        api_service: Option<Arc<dyn easytier::rpc_service::InstanceRpcService>>,
    ) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let is_running = { status.read().await.is_running };
            if !is_running { break; }

            if let Some(ref api) = api_service {
                let ctrl = easytier::proto::rpc_types::controller::BaseController::default();
                if let Ok(peers_resp) = api
                    .get_peer_manage_service()
                    .list_peer(ctrl.clone(), easytier::proto::api::instance::ListPeerRequest::default())
                    .await
                {
                    let mut s = status.write().await;
                    s.connected_peers = peers_resp.peer_infos.len() as u32;
                    if let Some(ref my_info) = peers_resp.my_info {
                        if !my_info.ipv4_addr.is_empty() {
                            s.virtual_ip = Some(my_info.ipv4_addr.clone());
                        }
                    }
                }
            }
        }
    }

    impl RunningInstance {
        pub fn connected_peers(&self) -> Option<u32> {
            self.status.try_read().ok().map(|s| s.connected_peers)
        }

        pub fn virtual_ip(&self) -> Option<String> {
            self.status.try_read().ok().and_then(|s| s.virtual_ip.clone())
        }

        pub async fn get_config_content(&self) -> Option<String> {
            self.config_content.read().await.clone()
        }

        pub async fn get_peers(&self) -> Vec<PeerInfo> {
            self.status.read().await.peers.clone()
        }

        pub async fn get_status(&self) -> Result<NetworkStatus, String> {
            let s = self.status.read().await;
            Ok(NetworkStatus {
                space_id: self.instance_id,
                status: if s.is_running { "connected".into() } else { "disconnected".into() },
                virtual_ip: s.virtual_ip.clone(),
                latency_ms: Some(s.avg_latency_ms),
                connected_peers: s.connected_peers,
            })
        }

        pub async fn stop(&mut self) -> Result<Option<String>, String> {
            let latest_config = self.config_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
            if let Some(ref cfg) = latest_config {
                *self.config_content.write().await = Some(cfg.clone());
            }
            if self.instance.is_some() {
                self.instance.take();
            }
            let mut s = self.status.write().await;
            s.is_running = false;
            Ok(latest_config.or_else(|| self.config_content.try_read().ok().and_then(|c| c.clone())))
        }
    }
}
