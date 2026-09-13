pub mod config;
pub mod downloader;
pub mod github;
pub mod process;

pub mod launcher {
    pub use super::launcher_internal::*;
}

use dashmap::DashMap;
use uuid::Uuid;
use std::path::PathBuf;

pub use downloader::{EasyTierDownloader, BinarySource};
pub use process::EasyTierProcess;

/// EasyTier 管理器，管理多个网络实例（Desktop 使用子进程，Mobile 使用库）
pub struct EasyTierManager {
    /// 运行中的进程: space_id → process (Desktop)
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[allow(dead_code)]
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
    pub fn new(config_dir: PathBuf, app_data_dir: PathBuf, resource_dir: Option<&std::path::Path>) -> Self {
        let downloader = EasyTierDownloader::new(&app_data_dir, resource_dir);

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

        self.clear_all_instances().await?;

        let old_config_path = self.config_dir.join(format!("{}.toml", instance_id));
        if old_config_path.exists() {
            std::fs::remove_file(&old_config_path).map_err(|e| format!("删除旧 TOML 配置文件失败: {}", e))?;
        }

        let _ = self.downloader.ensure_binary().await.map_err(|e| {
            crate::log_error!(format!("EasyTierManager.start_network: 确保二进制失败: {}", e));
            e
        })?;

        let _ = self.generate_config(cfg, &instance_id, initial_config.as_deref())?;

        let proto_cfg = self.build_proto_config(cfg, &instance_id);

        self.rpc_run_network_instance(&instance_id, &proto_cfg).await?;

        let config_path = self.config_dir.join(format!("{}.toml", instance_id));
        if config_path.exists() {
            std::fs::remove_file(&config_path).map_err(|e| format!("删除 RPC 写入的 TOML 文件失败: {}", e))?;
        }

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
        
        let effective_ipv4 = if !cfg.virtual_ipv4.is_empty() {
            Some(cfg.virtual_ipv4.clone())
        } else if let Some(ref ipv4) = cfg.ipv4 {
            if !ipv4.is_empty() { Some(ipv4.clone()) } else { None }
        } else {
            None
        };

        let default_space_ip = crate::config::get_str(crate::config::KEY_DEFAULT_SPACE_IP, crate::config::DEFAULT_SPACE_IP);

        let effective_ipv4 = effective_ipv4.or_else(|| {
            if !cfg.dhcp { Some(default_space_ip) } else { None }
        });

        NetworkConfig {
            instance_id: Some(instance_id.to_string()),
            network_name: if cfg.network_name.is_empty() { None } else { Some(cfg.network_name.clone()) },
            network_secret: if cfg.network_secret.is_empty() { None } else { Some(cfg.network_secret.clone()) },
            dhcp: Some(if effective_ipv4.is_some() { false } else { cfg.dhcp }),
            virtual_ipv4: effective_ipv4,
            hostname: cfg.hostname.clone(),
            listener_urls: cfg.listener_urls.clone(),
            peer_urls: if !cfg.peer_urls.is_empty() {
                cfg.peer_urls.clone()
            } else {
                cfg.peers.iter().map(|p| p.uri.clone()).collect()
            },
            proxy_cidrs: cfg.effective_proxy_cidrs(),
            enable_manual_routes: Some(cfg.enable_manual_routes),
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

        let rpc_port = crate::daemon::ipc::easytier_daemon_rpc_port();
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

    /// 停止网络实例（Desktop: 通过 RPC 调用）
    pub async fn stop_network(&self, instance_id: &Uuid) -> Result<Option<String>, String> {
        crate::log_info!(format!("EasyTierManager: 停止网络实例, id={}", instance_id));

        let config = self.read_config(instance_id);

        // 通过 RPC 删除网络实例（不杀 easytier-core 进程）
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::tunnel::tcp::TcpTunnelConnector;
        use easytier::proto::api::manage::{WebClientServiceClientFactory, DeleteNetworkInstanceRequest};

        let rpc_port = crate::daemon::ipc::easytier_daemon_rpc_port();
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

        // 删除对应的 TOML 配置文件，防止下次守护进程重启时自动恢复
        let config_path = self.config_dir.join(format!("{}.toml", instance_id));
        if config_path.exists() {
            std::fs::remove_file(&config_path).ok();
            crate::log_debug!("EasyTierManager: 已删除 TOML 文件 (stop), id={}", instance_id);
        }

        Ok(config)
    }

    /// 清除所有运行中的网络实例（通过 RPC），绕过 is_deletable() 检查
    /// 使用 retain_network_instance([]) 直接操作 HashMap::retain，不检查 deletability
    async fn clear_all_instances(&self) -> Result<(), String> {
        use std::time::Duration;
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::tunnel::tcp::TcpTunnelConnector;
        use easytier::proto::api::manage::{
            WebClientServiceClientFactory, RetainNetworkInstanceRequest,
        };

        let rpc_port = crate::daemon::ipc::easytier_daemon_rpc_port();
        let addr = format!("tcp://127.0.0.1:{}", rpc_port);
        let url: url::Url = addr.parse().map_err(|e| format!("解析 RPC 地址失败: {}", e))?;

        let connector = TcpTunnelConnector::new(url);
        let mut client = StandAloneClient::new(connector);

        let ctrl = BaseController::default();
        let manage_service = client
            .scoped_client::<WebClientServiceClientFactory<BaseController>>("".to_string())
            .await
            .map_err(|e| format!("连接 easytier-core RPC 失败: {}", e))?;

        // 带重试的 retain（easytier-core RPC 可能尚未就绪）
        for attempt in 1..=3 {
            match manage_service.retain_network_instance(
                ctrl.clone(),
                RetainNetworkInstanceRequest { inst_ids: vec![] },
            ).await {
                Ok(_) => {
                    return Ok(());
                }
                Err(e) => {
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    } else {
                        crate::log_warn!(format!("EasyTierManager.clear_all_instances: RPC 始终未就绪: {}", e));
                    }
                }
            }
        }

        Ok(())
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

        // 同步写入 JSON（完整 NetworkConfig，用于运行时 patch 读取）
        let json_path = self.config_dir.join(format!("{}.json", instance_id));
        let json_content = serde_json::to_string_pretty(cfg)
            .map_err(|e| format!("序列化 NetworkConfig 失败: {}", e))?;
        std::fs::write(&json_path, json_content)
            .map_err(|e| format!("写入 JSON 配置失败: {}", e))?;

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

    /// 通过 RPC 查询完整的运行时状态
    /// RPC 原始查询一次 collect_network_info（无 DHCP 等待循环）
    async fn query_rpc_status_once(&self, instance_id: &Uuid, rpc_port: u16) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::tunnel::tcp::TcpTunnelConnector;
        use easytier::proto::api::manage::WebClientServiceClientFactory;

        let addr = format!("tcp://127.0.0.1:{}", rpc_port);
        let url: url::Url = match addr.parse() {
            Ok(u) => u,
            Err(e) => {
                crate::log_warn!(format!("EasyTierManager: RPC 地址解析失败, addr={}, error={}", addr, e));
                return None;
            }
        };

        let connector = TcpTunnelConnector::new(url);
        let mut client = StandAloneClient::new(connector);

        let ctrl = BaseController::default();
        let web_service = match client.scoped_client::<WebClientServiceClientFactory<BaseController>>("".to_string()).await {
            Ok(svc) => svc,
            Err(e) => {
                crate::log_warn!(format!("EasyTierManager: RPC 连接失败, port={}, error={}", rpc_port, e));
                return None;
            }
        };

        let proto_uuid: easytier::proto::common::Uuid = (*instance_id).into();
        let inst_id_str = instance_id.to_string();
        let collect_req = easytier::proto::api::manage::CollectNetworkInfoRequest {
            inst_ids: vec![proto_uuid],
        };
        match web_service.collect_network_info(ctrl, collect_req).await {
            Ok(resp) => {
                let info_map = resp.info.as_ref();
                let running_info = info_map
                    .and_then(|m| m.map.get(&inst_id_str));

                match running_info {
                    Some(running_info) => {
                        // 实例启动失败时 easytier-core 返回带 error_msg 的 default 运行信息
                        // （my_node_info 为空、peer_route_pairs 为空），优先记录真实失败原因。
                        if let Some(ref err_msg) = running_info.error_msg {
                            crate::log_error!(format!(
                                "EasyTierManager: 实例启动失败, instance_id={}, error_msg={}",
                                instance_id, err_msg
                            ));
                            return Some(crate::daemon::ipc::SpaceRuntimeStatus {
                                space_id: instance_id.to_string(),
                                is_running: false,
                                virtual_ip: None,
                                connected_peers: 0,
                                rx_bytes: 0,
                                tx_bytes: 0,
                                avg_latency_ms: 0.0,
                            });
                        }

                        let mut virtual_ip = None;

                        if let Some(ref my_node) = running_info.my_node_info {
                            if let Some(ref ipv4_inet) = my_node.virtual_ipv4 {
                                if let Some(ref addr) = ipv4_inet.address {
                                    virtual_ip = Some(addr.to_string());
                                }
                            }
                        }
                        let connected_peers = running_info.peer_route_pairs.len() as u32;

                        crate::log_info!(format!(
                            "EasyTierManager: collect_network_info 成功, peer_route_pairs={}, routes={}, peers={}, connected_peers={}, has_virtual_ip={}",
                            running_info.peer_route_pairs.len(),
                            running_info.routes.len(),
                            running_info.peers.len(),
                            connected_peers,
                            virtual_ip.is_some()
                        ));

                        let mut total_latency = 0.0f64;
                        let mut latency_count = 0u32;
                        let mut total_rx_bytes = 0u64;
                        let mut total_tx_bytes = 0u64;

                        for pair in &running_info.peer_route_pairs {
                            for conn in &pair.peer.clone().unwrap().conns {
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
                    None => {
                        crate::log_warn!(format!("EasyTierManager: 实例不在 collect_network_info 响应中, instance_id={}", instance_id));
                        None
                    }
                }
            }
            Err(e) => {
                crate::log_warn!(format!("EasyTierManager: RPC 查询失败, port={}, error={}", rpc_port, e));
                None
            }
        }
    }

    /// 查询空间运行时状态（立即返回，不等待 DHCP）
    async fn query_rpc_status(&self, instance_id: &Uuid, rpc_port: u16) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        self.query_rpc_status_once(instance_id, rpc_port).await
    }

    /// 获取详细的网络统计信息
    pub async fn get_network_stats(&self, instance_id: &Uuid) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        let rpc_port = self.get_instance_rpc_port(instance_id)?;
        self.query_rpc_status(instance_id, rpc_port).await
    }

    /// 获取实例的 RPC 端口（共享 daemon 端口）
    fn get_instance_rpc_port(&self, _instance_id: &Uuid) -> Option<u16> {
        Some(crate::daemon::ipc::easytier_daemon_rpc_port())
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
    /// 使用 WebClientService::collect_network_info 获取合并的路由和连接信息
    async fn query_peer_list(&self, instance_id: &Uuid, rpc_port: u16) -> Option<Vec<crate::easytier::launcher::PeerInfo>> {
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::tunnel::tcp::TcpTunnelConnector;
        use easytier::proto::api::manage::WebClientServiceClientFactory;

        let addr = format!("tcp://127.0.0.1:{}", rpc_port);
        let url: url::Url = match addr.parse() {
            Ok(u) => u,
            Err(e) => {
                crate::log_warn!(format!("EasyTierManager: query_peer_list RPC 地址解析失败, addr={}, error={}", addr, e));
                return None;
            }
        };

        let max_retries = 3;
        for attempt in 1..=max_retries {
            let connector = TcpTunnelConnector::new(url.clone());
            let mut client = StandAloneClient::new(connector);

            let ctrl = BaseController::default();
            let web_service = client
                .scoped_client::<WebClientServiceClientFactory<BaseController>>("".to_string())
                .await;

            if let Ok(web_service) = web_service {
                let proto_uuid: easytier::proto::common::Uuid = (*instance_id).into();
                let inst_id_str = instance_id.to_string();

                let req = easytier::proto::api::manage::CollectNetworkInfoRequest {
                    inst_ids: vec![proto_uuid],
                };

                match web_service.collect_network_info(ctrl, req).await {
                    Ok(resp) => {
                        let running_info = match resp.info
                            .as_ref()
                            .and_then(|m| m.map.get(&inst_id_str))
                        {
                            Some(info) => info,
                            None => {
                                if attempt < max_retries {
                                    crate::log_warn!(format!("EasyTierManager: query_peer_list 实例未在 collect_network_info 响应中 (第{}/{}次), 即将重试", attempt, max_retries));
                                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                    continue;
                                }
                                crate::log_warn!(format!("EasyTierManager: query_peer_list 实例不在 collect_network_info 响应中 (已重试{}次), 返回空列表", max_retries));
                                return Some(Vec::new());
                            }
                        };

                        let local_peer_id = running_info.my_node_info
                            .as_ref()
                            .map(|n| n.peer_id);

                        let mut peer_infos = Vec::new();

                        if let Some(my_info) = running_info.my_node_info.as_ref() {
                            peer_infos.push(crate::easytier::launcher::PeerInfo {
                                peer_id: my_info.peer_id,
                                virtual_ip: Self::format_ipv4_inet(my_info.virtual_ipv4.as_ref()),
                                hostname: if my_info.hostname.is_empty() { None } else { Some(my_info.hostname.clone()) },
                                latency_ms: Some(0.0),
                                loss_rate: None,
                                rx_bytes: None,
                                tx_bytes: None,
                                connected: true,
                                is_local: true,
                                version: if my_info.version.is_empty() { None } else { Some(my_info.version.clone()) },
                                tunnel_proto: None,
                                nat_type: None,
                            });
                        }

                        if !running_info.peer_route_pairs.is_empty() {
                            for prp in &running_info.peer_route_pairs {
                                let route = match &prp.route {
                                    Some(r) => r,
                                    None => continue,
                                };
                                if local_peer_id.map(|id| id == route.peer_id).unwrap_or(false) {
                                    continue;
                                }
                                peer_infos.push(Self::peer_from_route_peer_pair(route, prp.peer.as_ref()));
                            }
                        } else {
                            for route in &running_info.routes {
                                if local_peer_id.map(|id| id == route.peer_id).unwrap_or(false) {
                                    continue;
                                }
                                peer_infos.push(Self::peer_from_route(route));
                            }
                        }

                        crate::log_info!(format!(
                            "EasyTierManager: query_peer_list 成功, result_peers={}, routes={}",
                            peer_infos.len(),
                            running_info.routes.len()
                        ));
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

    /// 从 Route 构建 PeerInfo（无连接统计）
    fn peer_from_route(route: &easytier::proto::api::instance::Route) -> crate::easytier::launcher::PeerInfo {
        let virtual_ip = Self::format_ipv4_inet(route.ipv4_addr.as_ref());

        let hostname = if route.hostname.is_empty() { None } else { Some(route.hostname.clone()) };
        let version = if route.version.is_empty() { None } else { Some(route.version.clone()) };

        // 从路由的 stun_info 提取 UDP NAT 类型（与 easytier PeerRoutePair::get_udp_nat_type 一致）
        let nat_type = route.stun_info.as_ref().map(|stun| {
            use easytier::proto::common::NatType;
            format!("{:?}", NatType::try_from(stun.udp_nat_type).unwrap_or(NatType::Unknown))
        });

        crate::easytier::launcher::PeerInfo {
            peer_id: route.peer_id,
            virtual_ip,
            hostname,
            latency_ms: Some(route.path_latency as f64),
            loss_rate: None,
            rx_bytes: None,
            tx_bytes: None,
            connected: true,
            is_local: false,
            version,
            tunnel_proto: None,
            nat_type,
        }
    }

    /// 格式化 Ipv4Inet 的地址为 a.b.c.d 字符串，0.0.0.0 返回 None
    fn format_ipv4_inet(inet: Option<&easytier::proto::common::Ipv4Inet>) -> Option<String> {
        inet
            .and_then(|i| i.address.as_ref())
            .map(|addr| format!("{}.{}.{}.{}",
                (addr.addr >> 24) & 0xFF,
                (addr.addr >> 16) & 0xFF,
                (addr.addr >> 8) & 0xFF,
                addr.addr & 0xFF
            ))
            .filter(|s| s != "0.0.0.0")
    }

    /// 从 Route + PeerInfo 合并构建 PeerInfo（含连接统计）
    fn peer_from_route_peer_pair(
        route: &easytier::proto::api::instance::Route,
        peer: Option<&easytier::proto::api::instance::PeerInfo>,
    ) -> crate::easytier::launcher::PeerInfo {
        let mut info = Self::peer_from_route(route);

        // 从路由的 stun_info 提取 UDP NAT 类型（与 easytier PeerRoutePair::get_udp_nat_type 一致）
        if let Some(stun) = &route.stun_info {
            use easytier::proto::common::NatType;
            info.nat_type = Some(format!(
                "{:?}",
                NatType::try_from(stun.udp_nat_type).unwrap_or(NatType::Unknown)
            ));
        }

        if let Some(peer) = peer {
            for conn in &peer.conns {
                if let Some(stats) = &conn.stats {
                    info.rx_bytes = Some(info.rx_bytes.unwrap_or(0) + stats.rx_bytes);
                    info.tx_bytes = Some(info.tx_bytes.unwrap_or(0) + stats.tx_bytes);

                    let conn_latency = stats.latency_us as f64 / 1000.0;
                    info.latency_ms = Some(match info.latency_ms {
                        Some(current) => current.min(conn_latency),
                        None => conn_latency,
                    });
                }
                info.loss_rate = Some(conn.loss_rate as f64);
                if conn.tunnel.is_some() {
                    info.tunnel_proto = conn.tunnel.as_ref().map(|t| t.tunnel_type.clone());
                }
            }
        }

        info
    }

    /// 获取空间运行时状态（通过 RPC 查询）
    pub async fn get_space_status(&self, instance_id: &Uuid) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        let rpc_port = self.get_instance_rpc_port(instance_id)?;
        self.query_rpc_status(instance_id, rpc_port).await
    }

    /// 获取 Mesh 可达的子网代理路由（桌面端: 通过 RPC 查询 collect_network_info 并提取 proxy_cidrs）
    pub async fn get_mesh_routes(&self, instance_id: &Uuid) -> Option<Vec<String>> {
        let rpc_port = self.get_instance_rpc_port(instance_id)?;
        self.query_mesh_routes(instance_id, rpc_port).await
    }

    /// 通过 RPC 查询 mesh routes（复用 collect_network_info）
    async fn query_mesh_routes(&self, instance_id: &Uuid, rpc_port: u16) -> Option<Vec<String>> {
        use easytier::proto::rpc_impl::standalone::StandAloneClient;
        use easytier::proto::rpc_types::controller::BaseController;
        use easytier::tunnel::tcp::TcpTunnelConnector;
        use easytier::proto::api::manage::WebClientServiceClientFactory;

        let url_str = format!("tcp://127.0.0.1:{}", rpc_port);
        let url = url_str.parse().ok()?;
        let connector = TcpTunnelConnector::new(url);
        let mut client = StandAloneClient::new(connector);

        let ctrl = BaseController::default();
        let web_service = client
            .scoped_client::<WebClientServiceClientFactory<BaseController>>("".to_string())
            .await
            .ok()?;

        let proto_uuid: easytier::proto::common::Uuid = (*instance_id).into();
        let inst_id_str = instance_id.to_string();

        let req = easytier::proto::api::manage::CollectNetworkInfoRequest {
            inst_ids: vec![proto_uuid],
        };

        let resp = web_service.collect_network_info(ctrl, req).await.ok()?;
        let running_info = resp.info.as_ref()?.map.get(&inst_id_str)?;

        let mut mesh_routes_set: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        // 从所有 Route.proxy_cidrs 提取
        for route in &running_info.routes {
            for cidr in &route.proxy_cidrs {
                mesh_routes_set.insert(cidr.clone());
            }
        }

        Some(mesh_routes_set.into_iter().collect())
    }

    /// 运行时修改配置（重启子进程应用新配置）
    pub async fn patch_config(
        &self,
        instance_id: &Uuid,
        patch: &serde_json::Value,
    ) -> Result<(), String> {
        crate::log_info!(format!("EasyTierManager: patch_config, id={}", instance_id));

        // 读取现有 NetworkConfig（优先 JSON，回退 TOML）
        let mut network_config = self.read_network_config(instance_id)?;

        // 解析 patch 中的字段并应用
        // patch 格式: { "network_name": "...", "network_secret": "...", "proxy_cidrs": [...], "flags": {...}, ... }
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
        if let Some(ipv6) = patch.get("ipv6").and_then(|v| v.as_str()) {
            network_config.ipv6 = Some(ipv6.to_string());
        }
        if let Some(hostname) = patch.get("hostname").and_then(|v| v.as_str()) {
            network_config.hostname = Some(hostname.to_string());
        }
        if let Some(virtual_ipv4) = patch.get("virtual_ipv4").and_then(|v| v.as_str()) {
            network_config.virtual_ipv4 = virtual_ipv4.to_string();
        }
        if let Some(network_length) = patch.get("network_length").and_then(|v| v.as_u64()) {
            network_config.network_length = network_length as u8;
        }
        if let Some(credential_file) = patch.get("credential_file").and_then(|v| v.as_str()) {
            network_config.credential_file = Some(credential_file.to_string());
        }
        if let Some(networking_method) = patch.get("networking_method").and_then(|v| v.as_i64()) {
            network_config.networking_method = networking_method as i32;
        }
        if let Some(public_server_url) = patch.get("public_server_url").and_then(|v| v.as_str()) {
            network_config.public_server_url = public_server_url.to_string();
        }
        if let Some(peer_urls) = patch.get("peer_urls").and_then(|v| v.as_array()) {
            network_config.peer_urls = peer_urls.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect();
        }
        if let Some(listener_urls) = patch.get("listener_urls").and_then(|v| v.as_array()) {
            network_config.listener_urls = listener_urls.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect();
        }
        if let Some(mapped_listeners) = patch.get("mapped_listeners").and_then(|v| v.as_array()) {
            network_config.mapped_listeners = mapped_listeners.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect();
        }
        if let Some(proxy_cidrs) = patch.get("proxy_cidrs").and_then(|v| v.as_array()) {
            network_config.proxy_cidrs = proxy_cidrs.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect();
        }
        if let Some(relay_network_whitelist) = patch.get("relay_network_whitelist").and_then(|v| v.as_array()) {
            network_config.relay_network_whitelist = relay_network_whitelist.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect();
        }
        if let Some(routes) = patch.get("routes").and_then(|v| v.as_array()) {
            network_config.routes = routes.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect();
        }
        if let Some(exit_nodes) = patch.get("exit_nodes").and_then(|v| v.as_array()) {
            network_config.exit_nodes = exit_nodes.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect();
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
        // 布尔标志位
        if let Some(enable_vpn_portal) = patch.get("enable_vpn_portal").and_then(|v| v.as_bool()) {
            network_config.enable_vpn_portal = enable_vpn_portal;
        }
        if let Some(disable_ipv6) = patch.get("disable_ipv6").and_then(|v| v.as_bool()) {
            network_config.disable_ipv6 = disable_ipv6;
        }
        if let Some(disable_p2p) = patch.get("disable_p2p").and_then(|v| v.as_bool()) {
            network_config.disable_p2p = disable_p2p;
        }
        if let Some(enable_kcp_proxy) = patch.get("enable_kcp_proxy").and_then(|v| v.as_bool()) {
            network_config.enable_kcp_proxy = enable_kcp_proxy;
        }
        if let Some(enable_quic_proxy) = patch.get("enable_quic_proxy").and_then(|v| v.as_bool()) {
            network_config.enable_quic_proxy = enable_quic_proxy;
        }
        if let Some(latency_first) = patch.get("latency_first").and_then(|v| v.as_bool()) {
            network_config.latency_first = latency_first;
        }
        if let Some(use_smoltcp) = patch.get("use_smoltcp").and_then(|v| v.as_bool()) {
            network_config.use_smoltcp = use_smoltcp;
        }
        if let Some(no_tun) = patch.get("no_tun").and_then(|v| v.as_bool()) {
            network_config.no_tun = no_tun;
        }
        if let Some(enable_exit_node) = patch.get("enable_exit_node").and_then(|v| v.as_bool()) {
            network_config.enable_exit_node = enable_exit_node;
        }
        if let Some(enable_socks5) = patch.get("enable_socks5").and_then(|v| v.as_bool()) {
            network_config.enable_socks5 = enable_socks5;
        }
        if let Some(enable_udp_broadcast_relay) = patch.get("enable_udp_broadcast_relay").and_then(|v| v.as_bool()) {
            network_config.enable_udp_broadcast_relay = enable_udp_broadcast_relay;
        }
        if let Some(enable_magic_dns) = patch.get("enable_magic_dns").and_then(|v| v.as_bool()) {
            network_config.enable_magic_dns = enable_magic_dns;
        }
        if let Some(enable_private_mode) = patch.get("enable_private_mode").and_then(|v| v.as_bool()) {
            network_config.enable_private_mode = enable_private_mode;
        }
        if let Some(enable_relay_network_whitelist) = patch.get("enable_relay_network_whitelist").and_then(|v| v.as_bool()) {
            network_config.enable_relay_network_whitelist = enable_relay_network_whitelist;
        }
        if let Some(enable_manual_routes) = patch.get("enable_manual_routes").and_then(|v| v.as_bool()) {
            network_config.enable_manual_routes = enable_manual_routes;
        }
        if let Some(bind_device) = patch.get("bind_device").and_then(|v| v.as_bool()) {
            network_config.bind_device = bind_device;
        }
        if let Some(p2p_only) = patch.get("p2p_only").and_then(|v| v.as_bool()) {
            network_config.p2p_only = p2p_only;
        }
        if let Some(lazy_p2p) = patch.get("lazy_p2p").and_then(|v| v.as_bool()) {
            network_config.lazy_p2p = lazy_p2p;
        }
        if let Some(multi_thread) = patch.get("multi_thread").and_then(|v| v.as_bool()) {
            network_config.multi_thread = multi_thread;
        }
        if let Some(proxy_forward_by_system) = patch.get("proxy_forward_by_system").and_then(|v| v.as_bool()) {
            network_config.proxy_forward_by_system = proxy_forward_by_system;
        }
        if let Some(disable_encryption) = patch.get("disable_encryption").and_then(|v| v.as_bool()) {
            network_config.disable_encryption = disable_encryption;
        }
        if let Some(disable_tcp_hole_punching) = patch.get("disable_tcp_hole_punching").and_then(|v| v.as_bool()) {
            network_config.disable_tcp_hole_punching = disable_tcp_hole_punching;
        }
        if let Some(disable_udp_hole_punching) = patch.get("disable_udp_hole_punching").and_then(|v| v.as_bool()) {
            network_config.disable_udp_hole_punching = disable_udp_hole_punching;
        }
        if let Some(disable_upnp) = patch.get("disable_upnp").and_then(|v| v.as_bool()) {
            network_config.disable_upnp = disable_upnp;
        }
        if let Some(disable_sym_hole_punching) = patch.get("disable_sym_hole_punching").and_then(|v| v.as_bool()) {
            network_config.disable_sym_hole_punching = disable_sym_hole_punching;
        }
        // 数值型
        if let Some(vpn_portal_listen_port) = patch.get("vpn_portal_listen_port").and_then(|v| v.as_u64()) {
            network_config.vpn_portal_listen_port = vpn_portal_listen_port as u16;
        }
        if let Some(vpn_portal_client_network_len) = patch.get("vpn_portal_client_network_len").and_then(|v| v.as_u64()) {
            network_config.vpn_portal_client_network_len = vpn_portal_client_network_len as u8;
        }
        if let Some(vpn_portal_client_network_addr) = patch.get("vpn_portal_client_network_addr").and_then(|v| v.as_str()) {
            network_config.vpn_portal_client_network_addr = vpn_portal_client_network_addr.to_string();
        }
        if let Some(socks5_port) = patch.get("socks5_port").and_then(|v| v.as_u64()) {
            network_config.socks5_port = socks5_port as u16;
        }
        if let Some(mtu) = patch.get("mtu").and_then(|v| v.as_u64()) {
            network_config.mtu = Some(mtu as u32);
        }
        if let Some(instance_recv_bps_limit) = patch.get("instance_recv_bps_limit").and_then(|v| v.as_u64()) {
            network_config.instance_recv_bps_limit = Some(instance_recv_bps_limit);
        }
        if let Some(dev_name) = patch.get("dev_name").and_then(|v| v.as_str()) {
            network_config.dev_name = dev_name.to_string();
        }

        // 重新生成配置文件（同时写入 TOML + JSON）
        let _ = self.generate_config(&network_config, instance_id, None)?;

        // 通过 RPC 重新启动网络实例（覆盖已有）
        let proto_cfg = self.build_proto_config(&network_config, instance_id);
        self.rpc_run_network_instance(instance_id, &proto_cfg).await?;
        crate::log_info!(format!("EasyTierManager: 配置已更新, id={}", instance_id));

        Ok(())
    }

    /// 读取空间的 NetworkConfig
    fn read_network_config(&self, instance_id: &Uuid) -> Result<config::NetworkConfig, String> {
        // 优先读取 JSON（完整 NetworkConfig 序列化）
        let json_path = self.config_dir.join(format!("{}.json", instance_id));
        if json_path.exists() {
            let json_content = std::fs::read_to_string(&json_path)
                .map_err(|e| format!("读取 JSON 配置失败: {}", e))?;
            let network_config: config::NetworkConfig = serde_json::from_str(&json_content)
                .map_err(|e| format!("反序列化 NetworkConfig 失败: {}", e))?;
            return Ok(network_config);
        }

        // 回退：读取 TOML 并仅解析 network_identity（兼容旧数据）
        let config_path = self.config_dir.join(format!("{}.toml", instance_id));
        if !config_path.exists() {
            return Err(format!("配置文件不存在: {}", config_path.display()));
        }

        let toml_content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("读取配置文件失败: {}", e))?;

        let mut network_config = config::NetworkConfig::default();

        if let Some(_ni) = toml_content.lines().find(|l| l.starts_with("[network_identity]")) {
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

    pub fn new(config_dir: PathBuf, app_data_dir: PathBuf, resource_dir: Option<&std::path::Path>) -> Self {
        let downloader = EasyTierDownloader::new(&app_data_dir, resource_dir);

        Self { instances: DashMap::new(), downloader, config_dir }
    }

    /// 启动网络实例（Mobile: 库方式）
    pub async fn start_network(
        &self,
        cfg: &config::NetworkConfig,
        instance_id: Uuid,
        initial_config: Option<String>,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<Uuid, String> {
        crate::log_info!(format!("EasyTierManager: 启动网络实例 (Mobile), network_name={}, id={}", cfg.network_name, instance_id));

        // 停止现有实例
        if self.instances.contains_key(&instance_id) {
            self.stop_network(&instance_id).await?;
        }

        // 重试启动：前一次实例可能未完全清理（Drop 不保证资源即时释放），最多重试 3 次
        let mut last_error = String::new();
        for attempt in 0..3 {
            if attempt > 0 {
                crate::log_warn!(format!("EasyTierManager: 启动重试 {}/3, id={}", attempt, instance_id));
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            match launcher_internal::start_easytier(cfg, instance_id, &self.config_dir, initial_config.clone(), app_handle.clone()).await {
                Ok(running) => {
                    self.instances.insert(instance_id, running);
                    crate::log_info!(format!("EasyTierManager: 网络实例已启动 (Mobile), id={}", instance_id));
                    return Ok(instance_id);
                }
                Err(e) => {
                    last_error = format!("尝试 {}/3 失败: {}", attempt + 1, e);
                    crate::log_warn!(last_error.clone());
                }
            }
        }
        Err(format!("启动网络实例失败 (Mobile, 重试耗尽): {}", last_error))
    }

    /// 停止网络实例
    pub async fn stop_network(&self, instance_id: &Uuid) -> Result<Option<String>, String> {
        crate::log_info!(format!("EasyTierManager: 停止网络实例 (Mobile), id={}", instance_id));
        if let Some((_, mut instance)) = self.instances.remove(instance_id) {
            let config = instance.stop().await?;
            crate::log_info!(format!("EasyTierManager: 网络实例已停止 (Mobile), id={}", instance_id));
            Ok(config)
        } else {
            crate::log_debug!(format!("EasyTierManager: 实例未找到 (Mobile, 可能已停止), id={}", instance_id));
            Ok(None)
        }
    }

    /// 获取网络状态
    pub async fn get_status(&self, instance_id: &Uuid) -> Result<crate::types::NetworkStatus, String> {
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
        match self.instances.get(instance_id) {
            Some(instance) => Ok(instance.get_peers().await),
            None => Ok(Vec::new()), // 实例不存在（过渡期）时返回空列表
        }
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
        Ok(easytier::VERSION.into())
    }

    /// 获取详细网络统计（Mobile: 基于库实例状态组装）
    pub async fn get_network_stats(&self, instance_id: &Uuid) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        let instance = self.instances.get(instance_id)?;
        let (is_running, virtual_ip, connected_peers, rx_bytes, tx_bytes, avg_latency_ms) = instance.get_runtime_stats().await;
        Some(crate::daemon::ipc::SpaceRuntimeStatus {
            space_id: instance_id.to_string(),
            is_running,
            virtual_ip,
            connected_peers,
            rx_bytes,
            tx_bytes,
            avg_latency_ms,
        })
    }

    /// 获取空间运行时状态（Mobile: 基于库实例状态组装）
    pub async fn get_space_status(&self, instance_id: &Uuid) -> Option<crate::daemon::ipc::SpaceRuntimeStatus> {
        self.get_network_stats(instance_id).await
    }

    /// 获取 Mesh 可达的子网代理路由（Mobile: 从实例状态获取）
    pub async fn get_mesh_routes(&self, instance_id: &Uuid) -> Option<Vec<String>> {
        let instance = self.instances.get(instance_id)?;
        Some(instance.get_mesh_routes().await)
    }

    /// 获取运行时快照（供 SpaceManager.connect 等待就绪使用）
    /// 返回: (is_running, virtual_ip, connected_peers)
    pub async fn get_runtime_snapshot(&self, instance_id: &Uuid) -> Option<(bool, Option<String>, u32)> {
        let instance = self.instances.get(instance_id)?;
        let (is_running, virtual_ip, connected_peers, _, _, _) = instance.get_runtime_stats().await;
        Some((is_running, virtual_ip, connected_peers))
    }

    /// 升级版本（Mobile 不支持）
    pub async fn upgrade(&self, _version: &str, _source: Option<BinarySource>) -> Result<(), String> {
        Err("Mobile 不支持版本升级".into())
    }

    /// 设置 TUN 文件描述符（移动端专用：从 VpnService/NetworkExtension 获取 fd 后注入）
    pub fn set_tun_fd(&self, instance_id: &Uuid, fd: i32) -> Result<(), String> {
        let entry = self.instances.get(instance_id)
            .ok_or_else(|| format!("Instance not found: {}", instance_id))?;
        
        let running_instance = entry.value();
        let instance = running_instance.instance.as_ref()
            .ok_or_else(|| "Running instance not initialized".to_string())?;
        
        let sender = instance.get_tun_fd_sender()
            .ok_or_else(|| "TUN fd sender not available".to_string())?;
        
        sender.try_send(Some(fd))
            .map_err(|e| format!("Failed to send TUN fd: {}", e))?;
        
        crate::log_info!(format!("EasyTierManager: TUN fd {} injected for instance {}", fd, instance_id));
        Ok(())
    }
}

mod launcher_internal {
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use std::path::PathBuf;
    use uuid::Uuid;
    use tauri::Emitter;
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
        pub instance: Option<easytier::launcher::NetworkInstance>,
    }

    struct InstanceStatus {
        virtual_ip: Option<String>,
        connected_peers: u32,
        is_running: bool,
        rx_bytes: u64,
        tx_bytes: u64,
        avg_latency_ms: f64,
        peers: Vec<PeerInfo>,
        // Mesh 可达的子网代理路由（从所有 Route.proxy_cidrs 去重聚合）
        mesh_routes: Vec<String>,
    }

    /// 启动 EasyTier 网络实例（库方式）
    pub async fn start_easytier(
        cfg: &config::NetworkConfig,
        instance_id: Uuid,
        config_dir: &PathBuf,
        initial_config: Option<String>,
        app_handle: Option<tauri::AppHandle>,
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
            #[cfg(any(target_os = "android", target_os = "ios"))]
            let err_msg = format!(
                "{}（移动端需要先通过 set_tun_fd 注入 TUN 文件描述符）",
                err_msg
            );
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
            mesh_routes: Vec::new(),
        }));

        let status_poll = status.clone();
        let stop_notifier = instance.get_stop_notifier();
        let instance_id_for_poll = instance_id;
        let app_handle_for_poll = app_handle.clone();
        tokio::spawn(async move {
            poll_instance_status(status_poll, api_service, instance_id_for_poll, app_handle_for_poll).await;
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
        instance_id: Uuid,
        app_handle: Option<tauri::AppHandle>,
    ) {
        // 首次快速轮询（500ms），前 10s (20次) 每 500ms 快速轮询，后续每 2 秒轮询一次
        let mut fast_poll_count = 0;
        let mut last_mesh_routes: Vec<String> = Vec::new();

        loop {
            if fast_poll_count < 20 { // 前 10s (20 * 500ms) 快速轮询
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                fast_poll_count += 1;
            } else {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            let is_running = { status.read().await.is_running };
            if !is_running { break; }

            if let Some(ref api) = api_service {
                let ctrl = easytier::proto::rpc_types::controller::BaseController::default();

                // 1) list_peer: 获取 peer 连接状态 + 本机 IP
                let peers_resp = api
                    .get_peer_manage_service()
                    .list_peer(ctrl.clone(), easytier::proto::api::instance::ListPeerRequest::default())
                    .await;

                // 2) list_route: 获取每个 peer 的 virtual_ip / hostname / latency
                let routes_resp = api
                    .get_peer_manage_service()
                    .list_route(ctrl.clone(), easytier::proto::api::instance::ListRouteRequest::default())
                    .await;

                if let Ok(peers_resp) = peers_resp {
                    // 按 peer_id 建立连接状态索引
                    use std::collections::HashMap;
                    let mut conn_map: HashMap<u32, &easytier::proto::api::instance::PeerInfo> = HashMap::new();
                    for p in &peers_resp.peer_infos {
                        conn_map.insert(p.peer_id, p);
                    }

                    // 本机 IP（my_info.ipv4_addr）
                    let my_ip = peers_resp.my_info.as_ref().and_then(|m| {
                        if m.ipv4_addr.is_empty() { None } else { Some(m.ipv4_addr.clone()) }
                    });

                    // 按 peer_id 建立 route 索引（virtual_ip / hostname / latency）
                    let mut route_map: HashMap<u32, &easytier::proto::api::instance::Route> = HashMap::new();
                    if let Ok(routes_resp) = &routes_resp {
                        for r in &routes_resp.routes {
                            route_map.insert(r.peer_id, r);
                        }
                    }

                    // 计算聚合统计：总接收/发送字节、平均延迟
                    let total_rx: u64 = conn_map.values().filter_map(|c| c.conns.first().and_then(|cn| cn.stats.as_ref().map(|s| s.rx_bytes))).sum();
                    let total_tx: u64 = conn_map.values().filter_map(|c| c.conns.first().and_then(|cn| cn.stats.as_ref().map(|s| s.tx_bytes))).sum();
                    let latency_sum: f64 = route_map.values().filter_map(|r| (r.path_latency > 0).then_some(r.path_latency as f64)).sum();
                    let latency_count = route_map.values().filter(|r| r.path_latency > 0).count();
                    let avg_latency = if latency_count > 0 { latency_sum / latency_count as f64 } else { 0.0 };

                    // 组装 PeerInfo 列表（连接的 peer + 有 route 的 peer 取并集）
                    let mut all_peer_ids: std::collections::HashSet<u32> = conn_map.keys().copied().collect();
                    all_peer_ids.extend(route_map.keys().copied());

                    let mut peers: Vec<PeerInfo> = all_peer_ids.iter().map(|&pid| {
                        let conn = conn_map.get(&pid);
                        let route = route_map.get(&pid);
                        PeerInfo {
                            peer_id: pid,
                            virtual_ip: route.and_then(|r| r.ipv4_addr.map(|ip| ip.to_string())),
                            hostname: route.and_then(|r| {
                                if r.hostname.is_empty() { None } else { Some(r.hostname.clone()) }
                            }),
                            latency_ms: route.map(|r| r.path_latency as f64).filter(|l| *l > 0.0),
                            loss_rate: conn.and_then(|c| c.conns.first().map(|cn| cn.loss_rate as f64)),
                            rx_bytes: conn.and_then(|c| c.conns.first().and_then(|cn| cn.stats.as_ref().map(|s| s.rx_bytes))),
                            tx_bytes: conn.and_then(|c| c.conns.first().and_then(|cn| cn.stats.as_ref().map(|s| s.tx_bytes))),
                            connected: conn.is_some(),
                            is_local: false,
                            version: route.and_then(|r| {
                                if r.version.is_empty() { None } else { Some(r.version.clone()) }
                            }),
                            tunnel_proto: conn.and_then(|c| c.conns.first().and_then(|cn| cn.tunnel.as_ref().map(|t| t.tunnel_type.clone()))),
                            nat_type: None,
                        }
                    }).collect();

                    // 添加本地节点（my_info）作为第一个元素，is_local = true
                    if let Some(my_info) = &peers_resp.my_info {
                        if !my_info.ipv4_addr.is_empty() {
                            peers.insert(0, PeerInfo {
                                peer_id: my_info.peer_id,
                                virtual_ip: Some(my_info.ipv4_addr.clone()),
                                hostname: if my_info.hostname.is_empty() { None } else { Some(my_info.hostname.clone()) },
                                latency_ms: Some(0.0),
                                loss_rate: None,
                                rx_bytes: None,
                                tx_bytes: None,
                                connected: true,
                                is_local: true,
                                version: if my_info.version.is_empty() { None } else { Some(my_info.version.clone()) },
                                tunnel_proto: None,
                                nat_type: None,
                            });
                        }
                    }

                    // 按 latency 排序（无 latency 的排后面），本地节点因 latency=0 会排在前面
                    peers.sort_by(|a, b| {
                        let la = a.latency_ms.unwrap_or(f64::MAX);
                        let lb = b.latency_ms.unwrap_or(f64::MAX);
                        la.partial_cmp(&lb).unwrap_or(std::cmp::Ordering::Equal)
                    });

                    // 提取并去重 mesh 子网代理路由（所有 Route.proxy_cidrs 聚合）
                    let mut mesh_routes_set: std::collections::HashSet<String> = std::collections::HashSet::new();
                    if let Ok(routes_resp) = &routes_resp {
                        for r in &routes_resp.routes {
                            for cidr in &r.proxy_cidrs {
                                mesh_routes_set.insert(cidr.clone());
                            }
                        }
                    }
                    // 也包含本地节点的 proxy_cidrs
                    if let Some(my_info) = &peers_resp.my_info {
                        for cidr in &my_info.proxy_cidrs {
                            mesh_routes_set.insert(cidr.clone());
                        }
                    }
                    let mesh_routes: Vec<String> = mesh_routes_set.into_iter().collect();

                    // 检测 mesh routes 变化并发送事件
                    // 首次采集到非空 mesh_routes 时也强制触发（last_mesh_routes 初始为空）
                    let is_first_non_empty = last_mesh_routes.is_empty() && !mesh_routes.is_empty();
                    if mesh_routes != last_mesh_routes || is_first_non_empty {
                        last_mesh_routes = mesh_routes.clone();
                        if let Some(ref handle) = app_handle {
                            let payload = serde_json::json!({
                                "spaceId": instance_id.to_string(),
                                "routes": mesh_routes,
                            });
                            let _ = handle.emit("mesh_routes_updated", payload);
                        }
                    }

                    let mut s = status.write().await;
                    s.connected_peers = peers_resp.peer_infos.len() as u32;
                    s.rx_bytes = total_rx;
                    s.tx_bytes = total_tx;
                    s.avg_latency_ms = avg_latency;
                    s.peers = peers;
                    s.mesh_routes = mesh_routes;
                    if let Some(ip) = my_ip {
                        s.virtual_ip = Some(ip);
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

        /// 获取聚合统计（供 get_network_stats 使用）
        pub async fn get_runtime_stats(&self) -> (bool, Option<String>, u32, u64, u64, f64) {
            let s = self.status.read().await;
            (s.is_running, s.virtual_ip.clone(), s.connected_peers, s.rx_bytes, s.tx_bytes, s.avg_latency_ms)
        }

        /// 获取 Mesh 可达的子网代理路由（去重后的所有 proxy_cidrs）
        pub async fn get_mesh_routes(&self) -> Vec<String> {
            self.status.read().await.mesh_routes.clone()
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
