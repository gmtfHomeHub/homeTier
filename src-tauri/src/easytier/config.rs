use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// EasyTier 网络配置（对应前端 EasyTierConfig 接口）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    // === Meta ===
    pub target_os: Option<String>,

    // === Basic ===
    pub instance_name: Option<String>,
    pub hostname: Option<String>,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub dhcp: bool,
    pub ipv6_public_addr_provider: Option<bool>,
    pub ipv6_public_addr_auto: Option<bool>,
    pub ipv6_public_addr_prefix: Option<String>,

    // === Network Identity ===
    pub network_name: String,
    pub network_secret: String,

    // === Connections ===
    pub networking_method: Option<String>,
    pub peers: Vec<PeerConfig>,
    pub listeners: Vec<String>,
    pub mapped_listeners: Vec<String>,

    // === Proxy & Routes ===
    pub proxy_networks: Vec<ProxyNetworkConfig>,
    pub routes: Vec<String>,
    pub exit_nodes: Vec<String>,

    // === VPN Portal ===
    pub vpn_portal: Option<VpnPortalConfig>,

    // === Port Forward ===
    pub port_forwards: Vec<PortForwardConfig>,

    // === Advanced Flags ===
    pub flags: HashMap<String, String>,

    // === Logging ===
    pub file_logger: Option<LogConfig>,
    pub console_logger: Option<LogConfig>,
}

/// Peer 节点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub uri: String,
    pub peer_public_key: Option<String>,
}

/// 子网代理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyNetworkConfig {
    pub cidr: String,
    pub mapped_cidr: Option<String>,
    pub allow: Option<Vec<String>>,
}

/// 端口转发配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForwardConfig {
    pub bind_addr: String,
    pub dst_addr: String,
    pub proto: String,
}

/// VPN Portal 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnPortalConfig {
    pub client_cidr: String,
    pub wireguard_listen: String,
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: Option<String>,
    pub file: Option<String>,
    pub dir: Option<String>,
    pub size_mb: Option<u32>,
    pub count: Option<u32>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            target_os: None,
            instance_name: None,
            hostname: None,
            ipv4: None,
            ipv6: None,
            dhcp: false,
            ipv6_public_addr_provider: None,
            ipv6_public_addr_auto: None,
            ipv6_public_addr_prefix: None,
            network_name: String::new(),
            network_secret: String::new(),
            networking_method: None,
            peers: Vec::new(),
            listeners: Vec::new(),
            mapped_listeners: Vec::new(),
            proxy_networks: Vec::new(),
            routes: Vec::new(),
            exit_nodes: Vec::new(),
            vpn_portal: None,
            port_forwards: Vec::new(),
            flags: HashMap::new(),
            file_logger: None,
            console_logger: None,
        }
    }
}

impl NetworkConfig {
    /// 从 config_json JSON 字符串安全映射到 NetworkConfig。
    /// - 空字符串 → 默认 NetworkConfig（所有字段为类型默认值）
    /// - 有效 JSON（partial）→ 缺失字段使用 Default::default()
    /// - 无效 JSON → 返回错误
    pub fn from_config_json(json_str: &str) -> Result<Self, String> {
        let trimmed = json_str.trim();
        if trimmed.is_empty() {
            return Ok(Self::default());
        }
        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("config_json 不是有效的 JSON: {}", e))?;
        serde_json::from_value(value)
            .map_err(|e| format!("config_json 映射到 NetworkConfig 失败: {}", e))
    }

    /// 转换为 EasyTier 的 TomlConfigLoader
    pub fn to_easytier_config(&self) -> Result<easytier::common::config::TomlConfigLoader, String> {
        use easytier::common::config::ConfigLoader;
        use easytier::common::config::NetworkIdentity;
        use easytier::common::config::PeerConfig as EasyPeerConfig;

        crate::log_debug!(format!("NetworkConfig -> TomlConfigLoader: name={}, dhcp={}, peers={}, listeners={}",
            self.network_name, self.dhcp, self.peers.len(), self.listeners.len()));

        let cfg = easytier::common::config::TomlConfigLoader::default();

        // 网络标识
        cfg.set_network_identity(NetworkIdentity::new(
            self.network_name.clone(),
            self.network_secret.clone(),
        ));

        // 实例名
        if let Some(ref name) = self.instance_name {
            cfg.set_inst_name(name.clone());
        }

        // 主机名
        if let Some(ref hostname) = self.hostname {
            cfg.set_hostname(Some(hostname.clone()));
        }

        // DHCP / 静态 IP
        cfg.set_dhcp(self.dhcp);

        // IPv4
        if let Some(ref ipv4) = self.ipv4 {
            if !ipv4.is_empty() {
                let inet: cidr::Ipv4Inet = format!("{}/24", ipv4)
                    .parse()
                    .map_err(|e| format!("Invalid IPv4: {}", e))?;
                cfg.set_ipv4(Some(inet));
            }
        }

        // IPv6
        if let Some(ref ipv6) = self.ipv6 {
            if !ipv6.is_empty() {
                cfg.set_ipv6(Some(ipv6.parse().map_err(|e| format!("Invalid IPv6: {}", e))?));
            }
        }

        // IPv6 公共地址
        if let Some(v) = self.ipv6_public_addr_provider {
            cfg.set_ipv6_public_addr_provider(v);
        }
        if let Some(v) = self.ipv6_public_addr_auto {
            cfg.set_ipv6_public_addr_auto(v);
        }
        if let Some(ref prefix) = self.ipv6_public_addr_prefix {
            if !prefix.is_empty() {
                cfg.set_ipv6_public_addr_prefix(Some(prefix.parse().map_err(|e| format!("Invalid IPv6 CIDR: {}", e))?));
            }
        }

        // 节点列表
        if !self.peers.is_empty() {
            let easy_peers: Vec<EasyPeerConfig> = self
                .peers
                .iter()
                .filter(|p| !p.uri.is_empty())
                .map(|p| {
                    p.uri.parse::<url::Url>()
                        .map(|u| EasyPeerConfig {
                            uri: u,
                            peer_public_key: p.peer_public_key.clone(),
                        })
                        .map_err(|e| format!("无效的 peer URI '{}': {}", p.uri, e))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !easy_peers.is_empty() {
                cfg.set_peers(easy_peers);
            }
        }

        // 监听地址
        if !self.listeners.is_empty() {
            let urls: Vec<url::Url> = self
                .listeners
                .iter()
                .filter(|l| !l.is_empty())
                .map(|l| l.parse::<url::Url>()
                    .map_err(|e| format!("无效的 listener '{}': {}", l, e)))
                .collect::<Result<Vec<_>, String>>()?;
            if !urls.is_empty() {
                cfg.set_listeners(urls);
            }
        }

        // 子网代理
        for proxy in &self.proxy_networks {
            if !proxy.cidr.is_empty() {
                if let Ok(cidr) = proxy.cidr.parse::<cidr::Ipv4Cidr>() {
                    let _ = cfg.add_proxy_cidr(cidr, None);
                }
            }
        }

        // 路由
        if !self.routes.is_empty() {
            let routes: Vec<cidr::Ipv4Cidr> = self
                .routes
                .iter()
                .filter_map(|r| r.parse().ok())
                .collect();
            if !routes.is_empty() {
                cfg.set_routes(Some(routes));
            }
        }

        // 出口节点
        if !self.exit_nodes.is_empty() {
            let nodes: Vec<std::net::IpAddr> = self
                .exit_nodes
                .iter()
                .filter_map(|n| n.parse().ok())
                .collect();
            if !nodes.is_empty() {
                cfg.set_exit_nodes(nodes);
            }
        }

        // 标志位
        let mut flags = cfg.get_flags();
        if let Some(v) = self.flags.get("enable_kcp_proxy") {
            flags.enable_kcp_proxy = v == "true";
        }
        if let Some(v) = self.flags.get("enable_quic_proxy") {
            flags.enable_quic_proxy = v == "true";
        }
        if let Some(v) = self.flags.get("latency_first") {
            flags.latency_first = v == "true";
        }
        if let Some(v) = self.flags.get("mtu") {
            if let Ok(mtu) = v.parse::<u32>() {
                flags.mtu = mtu;
            }
        }
        if let Some(ref algo) = self.flags.get("encryption_algorithm") {
            flags.encryption_algorithm = algo.to_string();
        }
        if let Some(v) = self.flags.get("no_tun") {
            flags.no_tun = v == "true";
        }
        if let Some(v) = self.flags.get("disable_p2p") {
            flags.disable_p2p = v == "true";
        }
        if let Some(v) = self.flags.get("multi_thread") {
            flags.multi_thread = v == "true";
        }
        if let Some(v) = self.flags.get("bind_device") {
            flags.bind_device = v == "true";
        }
        if let Some(ref s) = self.flags.get("dev_name") {
            if !s.is_empty() {
                flags.dev_name = s.to_string();
            }
        }
        if let Some(v) = self.flags.get("enable_encryption") {
            flags.enable_encryption = v == "true";
        }
        if let Some(v) = self.flags.get("enable_ipv6") {
            flags.enable_ipv6 = v == "true";
        }
        if let Some(ref s) = self.flags.get("default_protocol") {
            if !s.is_empty() {
                flags.default_protocol = s.to_string();
            }
        }
        cfg.set_flags(flags);

        Ok(cfg)
    }
}