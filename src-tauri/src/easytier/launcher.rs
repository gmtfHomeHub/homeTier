use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::types::NetworkStatus;
use super::config;
use easytier::common::config::{ConfigFileControl, ConfigFilePermission, ConfigLoader};
use std::path::PathBuf;
use serde::Serialize;

/// 序列化后返回前端的 Peer 信息
#[derive(Debug, Clone, Serialize)]
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

/// 启动 EasyTier 网络实例
pub async fn start_easytier(
    cfg: &config::NetworkConfig,
    instance_id: Uuid,
    config_dir: &PathBuf,
    initial_config: Option<String>,
) -> Result<RunningInstance, String> {
    let network_name = cfg.network_name.clone();
    crate::log_info!(format!("start_easytier: 开始启动, network_name={}, instance_id={}", network_name, instance_id));

    // 从 NetworkConfig 生成 TomlConfigLoader（始终使用空间的基本配置）
    let easytier_cfg = cfg.to_easytier_config()?;
    crate::log_info!(format!("start_easytier: 基本配置已加载, network_name={}, dhcp={}", network_name, cfg.dhcp), &instance_id.to_string());

    // 如果传入了空间级配置（JSON 格式），解析 flags 并应用到配置
    if let Some(ref config_str) = initial_config {
        crate::log_info!("start_easytier: 应用空间级配置", &instance_id.to_string());
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(config_str) {
            // 应用 flags
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
                crate::log_info!("start_easytier: 已应用空间级配置 flags", &instance_id.to_string());
            }
            // 应用其他 JSON 字段
            if let Some(hostname) = json_val.get("hostname").and_then(|v| v.as_str()) {
                if !hostname.is_empty() {
                    easytier_cfg.set_hostname(Some(hostname.to_string()));
                }
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
            // 应用 network_identity（覆盖 cfg 中的值）
            if let Some(ni) = json_val.get("network_identity") {
                let nn = ni.get("network_name").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                let ns = ni.get("network_secret").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                if nn.is_some() || ns.is_some() {
                    easytier_cfg.set_network_identity(easytier::common::config::NetworkIdentity::new(
                        nn.unwrap_or("").to_string(),
                        ns.unwrap_or("").to_string(),
                    ));
                    crate::log_info!("start_easytier: 已应用空间级配置 network_identity", &instance_id.to_string());
                }
            }
            // 应用 peers
            if let Some(peers) = json_val.get("peers").and_then(|v| v.as_array()) {
                if !peers.is_empty() {
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
                    if !easy_peers.is_empty() {
                        easytier_cfg.set_peers(easy_peers);
                        crate::log_info!("start_easytier: 已应用空间级配置 peers", &instance_id.to_string());
                    }
                }
            }
            // 应用 listeners
            if let Some(listeners) = json_val.get("listeners").and_then(|v| v.as_array()) {
                if !listeners.is_empty() {
                    let urls: Vec<url::Url> = listeners
                        .iter()
                        .filter_map(|l| l.as_str().and_then(|s| s.parse::<url::Url>().ok()))
                        .collect();
                    if !urls.is_empty() {
                        easytier_cfg.set_listeners(urls);
                        crate::log_info!("start_easytier: 已应用空间级配置 listeners", &instance_id.to_string());
                    }
                }
            }
            // 应用 routes
            if let Some(routes) = json_val.get("routes").and_then(|v| v.as_array()) {
                if !routes.is_empty() {
                    let cidrs: Vec<cidr::Ipv4Cidr> = routes
                        .iter()
                        .filter_map(|r| r.as_str().and_then(|s| s.parse().ok()))
                        .collect();
                    if !cidrs.is_empty() {
                        easytier_cfg.set_routes(Some(cidrs));
                    }
                }
            }
            // 应用 exit_nodes
            if let Some(exit_nodes) = json_val.get("exit_nodes").and_then(|v| v.as_array()) {
                if !exit_nodes.is_empty() {
                    let ips: Vec<std::net::IpAddr> = exit_nodes
                        .iter()
                        .filter_map(|n| n.as_str().and_then(|s| s.parse().ok()))
                        .collect();
                    if !ips.is_empty() {
                        easytier_cfg.set_exit_nodes(ips);
                    }
                }
            }
        }
    }

    let config_content = easytier_cfg.dump();
    crate::log_info!(format!("start_easytier: 最终配置:\n{}", &config_content[..config_content.len().min(2000)]), &instance_id.to_string());

    // 创建状态
    let status = Arc::new(RwLock::new(InstanceStatus {
        virtual_ip: None,
        connected_peers: 0,
        is_running: false,
        rx_bytes: 0,
        tx_bytes: 0,
        avg_latency_ms: 0.0,
        peers: Vec::new(),
    }));

    // 生成临时配置文件（EasyTier 的 ConfigFileControl 需要文件路径）
    let config_file_name = format!("{}.toml", network_name.replace('/', "_"));
    let config_path = config_dir.join(&config_file_name);
    crate::log_info!(format!("生成配置文件目录: {}", config_dir.display()), &instance_id.to_string());
    std::fs::create_dir_all(config_dir)
        .map_err(|e| format!("创建配置目录失败: {}", e))?;
    std::fs::write(&config_path, &config_content)
        .map_err(|e| format!("写入配置文件失败: {}", e))?;
    crate::log_info!(format!("生成配置文件: {}", config_path.display()), &instance_id.to_string());

    // 保存当前配置内容用于回写 DB
    let config_content_ref = Arc::new(RwLock::new(Some(config_content)));

    // 创建并启动 EasyTier 实例（使用配置文件路径）
    let mut instance = easytier::launcher::NetworkInstance::new(
        easytier_cfg,
        ConfigFileControl::new(Some(config_path.clone()), ConfigFilePermission::from(0u8)),
    );

    crate::log_debug!("start_easytier: 调用 NetworkInstance::start()");
    instance
        .start()
        .map_err(|e| {
            let err_msg = format!("EasyTier 启动失败: {:?}", e);
            crate::log_error!(&err_msg);
            err_msg
        })?;

    crate::log_info!("EasyTier 实例已启动", &instance_id.to_string());

    // 等待 API 服务就绪（EasyTier 在后台线程中异步初始化 RPC 服务）
    let api_service = {
        let mut retries = 0;
        const MAX_RETRIES: u32 = 50; // 50 * 200ms = 10s 超时
        loop {
            let svc = instance.get_api_service();
            if svc.is_some() {
                break svc;
            }
            retries += 1;
            if retries >= MAX_RETRIES {
                // 检查是否有错误信息
                let err_msg = instance.get_latest_error_msg();
                let msg = format!("EasyTier RPC 服务启动超时 ({}s)", MAX_RETRIES * 200 / 1000);
                crate::log_error!(&msg, &instance_id.to_string());
                if let Some(ref e) = err_msg {
                    crate::log_error!(format!("EasyTier 错误: {}", e), &instance_id.to_string());
                }
                return Err(msg);
            }
            crate::log_debug!(format!("start_easytier: 等待 RPC 服务就绪... ({}/{})", retries, MAX_RETRIES), &instance_id.to_string());
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    };
    let stop_notifier = instance.get_stop_notifier();
    crate::log_info!("start_easytier: RPC 服务已就绪, api_service=可用, stop_notifier={}",
        if stop_notifier.is_some() { "可用" } else { "不可用" });

    // 更新状态为运行中
    {
        let mut s = status.write().await;
        s.is_running = true;
    }

    // 启动状态轮询任务
    let status_poll = status.clone();
    tokio::spawn(async move {
        crate::log_debug!("start_easytier: 状态轮询任务已启动");
        poll_instance_status(status_poll, api_service).await;
        crate::log_debug!("start_easytier: 状态轮询任务已退出");
    });

    // 启动停止监听任务
    let status_stop = status.clone();
    let id_str = instance_id.to_string();
    tokio::spawn(async move {
        if let Some(notifier) = stop_notifier {
            crate::log_debug!("start_easytier: 停止监听任务已启动, 等待停止信号");
            notifier.notified().await;
            let mut s = status_stop.write().await;
            s.is_running = false;
            crate::log_info!("EasyTier 实例已停止（通过停止通知器）", &id_str);
        } else {
            crate::log_warn!("start_easytier: 停止通知器不可用，实例可能无法正常停止");
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

/// 轮询实例状态（通过 RPC 获取实时信息）
async fn poll_instance_status(
    status: Arc<RwLock<InstanceStatus>>,
    api_service: Option<Arc<dyn easytier::rpc_service::InstanceRpcService>>,
) {
    let mut last_virtual_ip: Option<String> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let is_running = { status.read().await.is_running };
        if !is_running {
            break;
        }

        if let Some(ref api) = api_service {
            let ctrl = easytier::proto::rpc_types::controller::BaseController::default();

            // 使用 show_node_info 获取本机节点信息（包含虚拟 IP）
            let mut local_node_info: Option<easytier::proto::api::instance::NodeInfo> = None;
            if let Ok(info) = api
                .get_peer_manage_service()
                .show_node_info(ctrl.clone(), easytier::proto::api::instance::ShowNodeInfoRequest::default())
                .await
            {
                if let Some(node_info) = info.node_info {
                    let mut s = status.write().await;
                    if !node_info.ipv4_addr.is_empty() && last_virtual_ip.as_deref() != Some(&node_info.ipv4_addr) {
                        crate::log_info!(format!("RPC 轮询: 获取到虚拟 IP = {}", node_info.ipv4_addr));
                        last_virtual_ip = Some(node_info.ipv4_addr.clone());
                    }
                    if !node_info.ipv4_addr.is_empty() {
                        s.virtual_ip = Some(node_info.ipv4_addr.clone());
                    }
                    local_node_info = Some(node_info);
                }
            } else {
                crate::log_debug!("RPC 轮询: show_node_info 失败（实例可能还在启动中）");
            }

            // 获取 peer 列表和路由表以更新统计信息
            if let Ok(peers_resp) = api
                .get_peer_manage_service()
                .list_peer(ctrl.clone(), easytier::proto::api::instance::ListPeerRequest::default())
                .await
            {
                let mut s = status.write().await;
                let peer_count = peers_resp.peer_infos.len() as u32;
                s.connected_peers = peer_count;

                // 获取路由表以匹配每个 peer 的详细信息
                let routes_resp = api
                    .get_peer_manage_service()
                    .list_route(ctrl, easytier::proto::api::instance::ListRouteRequest::default())
                    .await
                    .ok();

                // 合并 peer 和 route 为 PeerRoutePair
                let routes = routes_resp.map(|r| r.routes).unwrap_or_default();
                let peer_route_pairs = easytier::proto::api::instance::list_peer_route_pair(
                    peers_resp.peer_infos,
                    routes,
                );

                // 通过每个 peer 的连接信息聚合统计
                let mut total_rx: u64 = 0;
                let mut total_tx: u64 = 0;
                let mut total_latency_us: u64 = 0;
                let mut latency_count: u32 = 0;
                let mut peer_list: Vec<PeerInfo> = Vec::new();

                // 获取本机信息
                let local_peer_id = peers_resp.my_info.as_ref().map(|m| m.peer_id).unwrap_or(0);

                for prp in &peer_route_pairs {
                    let route = prp.route.as_ref();
                    let peer = match prp.peer.as_ref() {
                        Some(p) => p,
                        None => continue,
                    };
                    let mut peer_rx: u64 = 0;
                    let mut peer_tx: u64 = 0;
                    let mut peer_latency_us: u64 = 0;
                    let mut peer_conn_count: u32 = 0;

                    for conn in &peer.conns {
                        if let Some(stats) = &conn.stats {
                            peer_rx += stats.rx_bytes;
                            peer_tx += stats.tx_bytes;
                            total_rx += stats.rx_bytes;
                            total_tx += stats.tx_bytes;
                            if stats.latency_us > 0 {
                                peer_latency_us = stats.latency_us;
                                total_latency_us += stats.latency_us;
                                latency_count += 1;
                            }
                            peer_conn_count += 1;
                        }
                    }

                    let is_local = peer.peer_id == local_peer_id;
                    let lat_ms = route
                        .and_then(|r| {
                            if r.cost == 1 { prp.get_latency_ms() } else { Some(r.path_latency_latency_first() as f64) }
                        })
                        .or_else(|| if peer_latency_us > 0 { Some(peer_latency_us as f64 / 1000.0) } else { None });

                    peer_list.push(PeerInfo {
                        peer_id: peer.peer_id,
                        virtual_ip: route.and_then(|r| r.ipv4_addr.as_ref()).map(|ip| ip.to_string()),
                        hostname: route.map(|r| r.hostname.clone()).filter(|h| !h.is_empty()),
                        latency_ms: lat_ms,
                        loss_rate: prp.get_loss_rate().map(|r| r * 100.0),
                        rx_bytes: Some(peer_rx),
                        tx_bytes: Some(peer_tx),
                        connected: peer_conn_count > 0,
                        is_local,
                        version: route.and_then(|r| Some(r.version.clone())).filter(|v| !v.is_empty()),
                        tunnel_proto: prp.get_conn_protos().map(|p| p.join(",")),
                        nat_type: {
                            let raw = prp.get_udp_nat_type();
                            if raw.is_empty() { None } else { Some(raw) }
                        },
                    });
                }

                // 更新虚拟 IP（从 my_info）
                if let Some(ref my_info) = peers_resp.my_info {
                    if !my_info.ipv4_addr.is_empty() && last_virtual_ip.as_deref() != Some(&my_info.ipv4_addr) {
                        crate::log_info!(format!("RPC 轮询: 获取到虚拟 IP = {}", my_info.ipv4_addr));
                        last_virtual_ip = Some(my_info.ipv4_addr.clone());
                    }
                    if !my_info.ipv4_addr.is_empty() {
                        s.virtual_ip = Some(my_info.ipv4_addr.clone());
                    }
                }

                s.peers = peer_list;

                // 确保本机记录存在（始终包含本机），优先使用 show_node_info 的结果
                let local_info = local_node_info.as_ref().or_else(|| {
                    let s = status.blocking_read();
                    s.virtual_ip.as_ref().map(|_| {
                        // 如果 local_node_info 不可用，尝试从 peers_resp.my_info 获取
                        peers_resp.my_info.as_ref()
                    }).flatten()
                }).or_else(|| peers_resp.my_info.as_ref());

                if let Some(ref info) = local_info {
                    // 查找是否已有本机记录，没有则添加
                    let has_local = s.peers.iter().any(|p| p.is_local);
                    if !has_local {
                        let nat = info.stun_info.as_ref().map(|s| {
                            use easytier::proto::common::NatType;
                            let nt = NatType::try_from(s.udp_nat_type).unwrap_or(NatType::Unknown);
                            format!("{:?}", nt)
                        });
                        // 插入到列表最前面，使本机排在第一行
                        s.peers.insert(0, PeerInfo {
                            peer_id: info.peer_id,
                            virtual_ip: if info.ipv4_addr.is_empty() { None } else { Some(info.ipv4_addr.clone()) },
                            hostname: if info.hostname.is_empty() { None } else { Some(info.hostname.clone()) },
                            latency_ms: None,
                            loss_rate: None,
                            rx_bytes: None,
                            tx_bytes: None,
                            connected: true,
                            is_local: true,
                            version: if info.version.is_empty() { None } else { Some(info.version.clone()) },
                            tunnel_proto: None,
                            nat_type: nat,
                        });
                    }
                }

                // 每次轮询都记录 peer 信息
                if peer_count > 0 {
                    let details: Vec<String> = s.peers.iter().map(|p| {
                        format!("#{} ip={:?} lat={:?}ms rx={:?} tx={:?}",
                            p.peer_id, p.virtual_ip, p.latency_ms, p.rx_bytes, p.tx_bytes)
                    }).collect();
                    crate::log_info!(format!("RPC 轮询: peers={} 详情: [{}]", peer_count, details.join(", ")));
                } else {
                    crate::log_debug!("RPC 轮询: 当前无在线 peer");
                }

                let old_rx = s.rx_bytes;
                let old_tx = s.tx_bytes;
                s.rx_bytes = total_rx;
                s.tx_bytes = total_tx;
                s.avg_latency_ms = if latency_count > 0 {
                    total_latency_us as f64 / latency_count as f64 / 1000.0
                } else {
                    0.0
                };

                // 仅当流量变化超过 1MB 时记录，避免日志刷屏
                if total_rx.abs_diff(old_rx) > 1_000_000 || total_tx.abs_diff(old_tx) > 1_000_000 {
                    crate::log_debug!(format!("RPC 轮询: 流量 rx={}MB tx={}MB 延迟={:.1}ms peers={}",
                        total_rx / 1_000_000, total_tx / 1_000_000, s.avg_latency_ms, peer_count));
                }
            } else {
                crate::log_debug!("RPC 轮询: list_peer 失败（可能无 peer 连接）");
            }
        } else {
            crate::log_warn!("RPC 轮询: api_service 不可用，无法获取运行时状态");
        }
    }
}

impl RunningInstance {
    /// 获取 connected_peers 数量的快照（非阻塞）
    pub fn connected_peers(&self) -> Option<u32> {
        self.status.try_read().ok().map(|s| s.connected_peers)
    }

    /// 获取 virtual_ip 的快照（非阻塞）
    pub fn virtual_ip(&self) -> Option<String> {
        self.status
            .try_read()
            .ok()
            .and_then(|s| s.virtual_ip.clone())
    }

    /// 获取当前配置内容（TOML 字符串）
    pub async fn get_config_content(&self) -> Option<String> {
        self.config_content.read().await.clone()
    }

    /// 获取 peer 列表
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.status.read().await.peers.clone()
    }

    /// 获取网络状态
    pub async fn get_status(&self) -> Result<NetworkStatus, String> {
        let s = self.status.read().await;
        Ok(NetworkStatus {
            space_id: self.instance_id,
            status: if s.is_running {
                "connected".into()
            } else {
                "disconnected".into()
            },
            virtual_ip: s.virtual_ip.clone(),
            latency_ms: Some(s.avg_latency_ms),
            connected_peers: s.connected_peers,
        })
    }

    /// 停止网络实例，并返回最新配置内容
    pub async fn stop(&mut self) -> Result<Option<String>, String> {
        crate::log_info!("EasyTier 实例停止", &self.instance_id.to_string());
        // 停止前先从配置文件读取最新内容
        let latest_config = self.config_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
        if let Some(ref cfg) = latest_config {
            *self.config_content.write().await = Some(cfg.clone());
            crate::log_debug!("stop: 已读取配置文件最新内容");
        }

        // 直接 drop NetworkInstance 触发 EasyTierLauncher::drop()
        if self.instance.is_some() {
            crate::log_debug!("stop: 丢弃 NetworkInstance 触发 EasyTier 清理");
            self.instance.take();
        }
        let mut s = self.status.write().await;
        s.is_running = false;
        crate::log_info!("EasyTier 实例已完全停止", &self.instance_id.to_string());

        // 返回最新配置（优先使用文件内容，其次使用内存中的）
        let result = latest_config.or_else(|| {
            self.config_content.try_read().ok().and_then(|c| c.clone())
        });
        Ok(result)
    }
}