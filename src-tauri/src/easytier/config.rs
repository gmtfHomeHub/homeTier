use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

/// Frontend-facing NetworkConfig — mirrors src/types/network.ts NetworkConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    // === Instance ===
    pub instance_id: String,
    pub instance_name: Option<String>,

    // === Basic ===
    pub dhcp: bool,
    pub virtual_ipv4: String,
    pub network_length: u8,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub hostname: Option<String>,
    pub network_name: String,
    pub network_secret: String,
    pub credential_file: Option<String>,

    // === Networking ===
    pub networking_method: i32,
    pub public_server_url: String,
    pub peer_urls: Vec<String>,

    // === Proxy ===
    pub proxy_cidrs: Vec<String>,

    // === VPN Portal ===
    pub enable_vpn_portal: bool,
    pub vpn_portal_listen_port: u16,
    pub vpn_portal_client_network_addr: String,
    pub vpn_portal_client_network_len: u8,

    // === Listeners ===
    pub listener_urls: Vec<String>,
    pub mapped_listeners: Vec<String>,

    // === Boolean Flags ===
    pub latency_first: bool,
    pub dev_name: String,
    pub use_smoltcp: bool,
    pub disable_ipv6: bool,
    pub ipv6_public_addr_provider: Option<bool>,
    pub ipv6_public_addr_auto: Option<bool>,
    pub ipv6_public_addr_prefix: Option<String>,
    pub enable_kcp_proxy: bool,
    pub disable_kcp_input: bool,
    pub enable_quic_proxy: bool,
    pub disable_quic_input: bool,
    pub disable_p2p: bool,
    pub p2p_only: bool,
    pub lazy_p2p: bool,
    pub bind_device: bool,
    pub no_tun: bool,
    pub enable_exit_node: bool,
    pub relay_all_peer_rpc: bool,
    pub need_p2p: bool,
    pub multi_thread: bool,
    pub proxy_forward_by_system: bool,
    pub disable_encryption: bool,
    pub disable_tcp_hole_punching: bool,
    pub disable_udp_hole_punching: bool,
    pub disable_upnp: bool,
    pub enable_udp_broadcast_relay: bool,
    pub disable_sym_hole_punching: bool,
    pub enable_relay_network_whitelist: bool,
    pub enable_magic_dns: bool,
    pub enable_private_mode: bool,
    pub enable_socks5: bool,

    // === Relay Whitelist ===
    pub relay_network_whitelist: Vec<String>,

    // === Routes ===
    pub enable_manual_routes: bool,
    pub routes: Vec<String>,

    // === Exit Nodes ===
    pub exit_nodes: Vec<String>,

    // === SOCKS5 ===
    pub socks5_port: u16,

    // === Misc ===
    pub mtu: Option<u32>,
    pub instance_recv_bps_limit: Option<u64>,

    // === Port Forwards ===
    pub port_forwards: Vec<PortForwardConfig>,

    // === ACL ===
    pub acl: Option<serde_json::Value>,

    // === Logging ===
    pub file_logger: Option<LogConfig>,
    pub console_logger: Option<LogConfig>,

    // === Legacy compat ===
    pub peers: Vec<PeerConfig>,
    pub listeners: Vec<String>,
    pub proxy_networks: Vec<ProxyNetworkConfig>,
    pub flags: HashMap<String, String>,
}

/// Port forward — matches frontend PortForwardConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForwardConfig {
    #[serde(default)]
    pub bind_ip: String,
    #[serde(default)]
    pub bind_port: u16,
    #[serde(default)]
    pub dst_ip: String,
    #[serde(default)]
    pub dst_port: u16,
    #[serde(default)]
    pub proto: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub uri: String,
    pub peer_public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyNetworkConfig {
    pub cidr: String,
    pub mapped_cidr: Option<String>,
    pub allow: Option<Vec<String>>,
}

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
            instance_id: uuid::Uuid::new_v4().to_string(),
            instance_name: None,
            dhcp: true,
            virtual_ipv4: String::new(),
            network_length: 24,
            ipv4: None,
            ipv6: None,
            hostname: None,
            network_name: "easytier".to_string(),
            network_secret: String::new(),
            credential_file: None,
            networking_method: 1,
            public_server_url: String::new(),
            peer_urls: Vec::new(),
            proxy_cidrs: Vec::new(),
            enable_vpn_portal: false,
            vpn_portal_listen_port: 22022,
            vpn_portal_client_network_addr: String::new(),
            vpn_portal_client_network_len: 24,
            listener_urls: vec![
                "tcp://0.0.0.0:11010".to_string(),
                "udp://0.0.0.0:11010".to_string(),
                "wg://0.0.0.0:11011".to_string(),
            ],
            mapped_listeners: Vec::new(),
            latency_first: false,
            dev_name: String::new(),
            use_smoltcp: false,
            disable_ipv6: false,
            ipv6_public_addr_provider: None,
            ipv6_public_addr_auto: None,
            ipv6_public_addr_prefix: None,
            enable_kcp_proxy: false,
            disable_kcp_input: false,
            enable_quic_proxy: false,
            disable_quic_input: false,
            disable_p2p: false,
            p2p_only: false,
            lazy_p2p: false,
            bind_device: true,
            no_tun: false,
            enable_exit_node: false,
            relay_all_peer_rpc: false,
            need_p2p: false,
            multi_thread: true,
            proxy_forward_by_system: false,
            disable_encryption: false,
            disable_tcp_hole_punching: false,
            disable_udp_hole_punching: false,
            disable_upnp: false,
            enable_udp_broadcast_relay: false,
            disable_sym_hole_punching: false,
            enable_relay_network_whitelist: false,
            enable_magic_dns: false,
            enable_private_mode: false,
            enable_socks5: false,
            relay_network_whitelist: Vec::new(),
            enable_manual_routes: false,
            routes: Vec::new(),
            exit_nodes: Vec::new(),
            socks5_port: 1080,
            mtu: None,
            instance_recv_bps_limit: None,
            port_forwards: Vec::new(),
            acl: None,
            file_logger: None,
            console_logger: None,
            peers: Vec::new(),
            listeners: Vec::new(),
            proxy_networks: Vec::new(),
            flags: HashMap::new(),
        }
    }
}

impl NetworkConfig {
    pub fn from_config_json(json_str: &str) -> Result<Self, String> {
        let trimmed = json_str.trim();
        if trimmed.is_empty() {
            return Ok(Self::default());
        }
        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("config_json is not valid JSON: {}", e))?;
        let keys: Vec<String> = match &value {
            serde_json::Value::Object(m) => m.keys().cloned().collect(),
            _ => vec![],
        };
        serde_json::from_value(value).map_err(|e| {
            format!(
                "config_json -> NetworkConfig failed: {} (keys: {:?}, json_preview: {}..)",
                e,
                keys,
                &trimmed[..trimmed.len().min(200)]
            )
        })
    }

    pub fn to_easytier_config(&self) -> Result<easytier::common::config::TomlConfigLoader, String> {
        use easytier::common::config::ConfigLoader;
        use easytier::common::config::NetworkIdentity;
        use easytier::common::config::PeerConfig as EasyPeerConfig;
        use std::net::ToSocketAddrs;
        use std::str::FromStr;

        crate::log_debug!(format!(
            "NetworkConfig -> TomlConfigLoader: name={}, dhcp={}, peer_urls={}",
            self.network_name,
            self.dhcp,
            self.peer_urls.len()
        ));

        let cfg = easytier::common::config::TomlConfigLoader::default();

        // Network identity
        cfg.set_network_identity(NetworkIdentity::new(
            self.network_name.clone(),
            self.network_secret.clone(),
        ));

        // Instance name — use instance_id as fallback
        let inst_name = if self.instance_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            self.instance_id.clone()
        };
        cfg.set_inst_name(inst_name);

        // Hostname
        if let Some(ref hostname) = self.hostname {
            cfg.set_hostname(Some(hostname.clone()));
        }

        // DHCP
        cfg.set_dhcp(self.dhcp);

        // IPv4 — combine virtual_ipv4 + network_length
        if !self.virtual_ipv4.is_empty() {
            let ipv4_str = format!("{}/{}", self.virtual_ipv4, self.network_length);
            if let Ok(inet) = cidr::Ipv4Inet::from_str(&ipv4_str) {
                cfg.set_ipv4(Some(inet));
            }
        }

        // IPv6
        cfg.set_ipv6(None);

        // IPv6 public address
        cfg.set_ipv6_public_addr_auto(self.ipv6_public_addr_auto.unwrap_or(false));

        // Peer URLs
        if !self.peer_urls.is_empty() {
            let easy_peers: Vec<EasyPeerConfig> = self
                .peer_urls
                .iter()
                .filter(|u| !u.is_empty())
                .map(|u| {
                    u.parse::<url::Url>()
                        .map(|url| EasyPeerConfig {
                            uri: url,
                            peer_public_key: None,
                        })
                        .map_err(|e| format!("invalid peer URL '{}': {}", u, e))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !easy_peers.is_empty() {
                cfg.set_peers(easy_peers);
            }
        }

        // Legacy peers
        if !self.peers.is_empty() && self.peer_urls.is_empty() {
            let easy_peers: Vec<EasyPeerConfig> = self
                .peers
                .iter()
                .filter(|p| !p.uri.is_empty())
                .map(|p| {
                    p.uri
                        .parse::<url::Url>()
                        .map(|url| EasyPeerConfig {
                            uri: url,
                            peer_public_key: p.peer_public_key.clone(),
                        })
                        .map_err(|e| format!("invalid peer URI '{}': {}", p.uri, e))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !easy_peers.is_empty() {
                cfg.set_peers(easy_peers);
            }
        }

        // Listeners
        if !self.listener_urls.is_empty() {
            let urls: Vec<url::Url> = self
                .listener_urls
                .iter()
                .filter(|l| !l.is_empty())
                .map(|l| {
                    l.parse::<url::Url>()
                        .map_err(|e| format!("invalid listener '{}': {}", l, e))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !urls.is_empty() {
                cfg.set_listeners(urls);
            }
        }

        // Legacy listeners
        if !self.listeners.is_empty() && self.listener_urls.is_empty() {
            let urls: Vec<url::Url> = self
                .listeners
                .iter()
                .filter(|l| !l.is_empty())
                .map(|l| {
                    l.parse::<url::Url>()
                        .map_err(|e| format!("invalid listener '{}': {}", l, e))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !urls.is_empty() {
                cfg.set_listeners(urls);
            }
        }

        // Mapped listeners
        if !self.mapped_listeners.is_empty() {
            let urls: Vec<url::Url> = self
                .mapped_listeners
                .iter()
                .filter(|l| !l.is_empty())
                .map(|l| {
                    l.parse::<url::Url>()
                        .map_err(|e| format!("invalid mapped listener '{}': {}", l, e))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !urls.is_empty() {
                cfg.set_mapped_listeners(Some(urls));
            }
        }

        // Proxy CIDRs
        if !self.proxy_cidrs.is_empty() {
            for cidr_str in &self.proxy_cidrs {
                if !cidr_str.is_empty() {
                    if let Ok(cidr) = cidr::Ipv4Cidr::from_str(cidr_str) {
                        let _ = cfg.add_proxy_cidr(cidr, None);
                    }
                }
            }
        }

        // Legacy proxy networks
        if !self.proxy_networks.is_empty() && self.proxy_cidrs.is_empty() {
            for proxy in &self.proxy_networks {
                if !proxy.cidr.is_empty() {
                    if let Ok(cidr) = cidr::Ipv4Cidr::from_str(&proxy.cidr) {
                        let mapped = proxy
                            .mapped_cidr
                            .as_ref()
                            .and_then(|m| cidr::Ipv4Cidr::from_str(m).ok());
                        let _ = cfg.add_proxy_cidr(cidr, mapped);
                    }
                }
            }
        }

        // Routes
        if self.enable_manual_routes && !self.routes.is_empty() {
            let routes: Vec<cidr::Ipv4Cidr> = self
                .routes
                .iter()
                .filter_map(|r| cidr::Ipv4Cidr::from_str(r).ok())
                .collect();
            if !routes.is_empty() {
                cfg.set_routes(Some(routes));
            }
        }

        // Exit nodes
        if !self.exit_nodes.is_empty() {
            let nodes: Vec<IpAddr> = self
                .exit_nodes
                .iter()
                .filter_map(|n| n.parse().ok())
                .collect();
            if !nodes.is_empty() {
                cfg.set_exit_nodes(nodes);
            }
        }

        // VPN Portal
        if self.enable_vpn_portal {
            let client_cidr_str = format!(
                "{}/{}",
                self.vpn_portal_client_network_addr, self.vpn_portal_client_network_len
            );
            if let Ok(client_cidr) = cidr::Ipv4Cidr::from_str(&client_cidr_str) {
                let wireguard_listen =
                    format!("0.0.0.0:{}", self.vpn_portal_listen_port)
                        .to_socket_addrs()
                        .ok()
                        .and_then(|mut addrs| addrs.next())
                        .unwrap_or(std::net::SocketAddr::V4(
                            std::net::SocketAddrV4::new(
                                std::net::Ipv4Addr::new(0, 0, 0, 0),
                                self.vpn_portal_listen_port,
                            ),
                        ));
                cfg.set_vpn_portal_config(
                    easytier::common::config::VpnPortalConfig {
                        client_cidr,
                        wireguard_listen,
                    },
                );
            }
        }

        // SOCKS5
        if self.enable_socks5 {
            let socks5_url = format!("socks5://0.0.0.0:{}", self.socks5_port);
            if let Ok(url) = socks5_url.parse::<url::Url>() {
                cfg.set_socks5_portal(Some(url));
            }
        }

        // Port forwards
        if !self.port_forwards.is_empty() {
            let forwards: Vec<easytier::common::config::PortForwardConfig> = self
                .port_forwards
                .iter()
                .filter_map(|pf| {
                    let bind_addr = format!("{}:{}", pf.bind_ip, pf.bind_port)
                        .to_socket_addrs()
                        .ok()
                        .and_then(|mut addrs| addrs.next())?;
                    let dst_addr = format!("{}:{}", pf.dst_ip, pf.dst_port)
                        .to_socket_addrs()
                        .ok()
                        .and_then(|mut addrs| addrs.next())?;
                    Some(easytier::common::config::PortForwardConfig {
                        bind_addr,
                        dst_addr,
                        proto: pf.proto.clone(),
                    })
                })
                .collect();
            if !forwards.is_empty() {
                cfg.set_port_forwards(forwards);
            }
        }

        // Credential file
        if let Some(ref path) = self.credential_file {
            if !path.is_empty() {
                cfg.set_credential_file(Some(std::path::PathBuf::from(path)));
            }
        }

        // === Flags ===
        let mut flags = cfg.get_flags();

        flags.latency_first = self.latency_first;
        flags.use_smoltcp = self.use_smoltcp;
        flags.enable_ipv6 = !self.disable_ipv6;
        flags.enable_kcp_proxy = self.enable_kcp_proxy;
        flags.disable_kcp_input = self.disable_kcp_input;
        flags.enable_quic_proxy = self.enable_quic_proxy;
        flags.disable_quic_input = self.disable_quic_input;
        flags.disable_p2p = self.disable_p2p;
        flags.p2p_only = self.p2p_only;
        flags.lazy_p2p = self.lazy_p2p;
        flags.bind_device = self.bind_device;
        flags.no_tun = self.no_tun;
        flags.enable_exit_node = self.enable_exit_node;
        flags.relay_all_peer_rpc = self.relay_all_peer_rpc;
        flags.need_p2p = self.need_p2p;
        flags.multi_thread = self.multi_thread;
        flags.proxy_forward_by_system = self.proxy_forward_by_system;
        flags.enable_encryption = !self.disable_encryption;
        flags.disable_tcp_hole_punching = self.disable_tcp_hole_punching;
        flags.disable_udp_hole_punching = self.disable_udp_hole_punching;
        flags.disable_upnp = self.disable_upnp;
        flags.enable_udp_broadcast_relay = self.enable_udp_broadcast_relay;
        flags.disable_sym_hole_punching = self.disable_sym_hole_punching;
        flags.accept_dns = self.enable_magic_dns;
        flags.private_mode = self.enable_private_mode;

        if self.enable_relay_network_whitelist && !self.relay_network_whitelist.is_empty() {
            flags.relay_network_whitelist = self.relay_network_whitelist.join(",");
        } else {
            flags.relay_network_whitelist = "*".to_string();
        }

        if !self.dev_name.is_empty() {
            flags.dev_name = self.dev_name.clone();
        }

        if let Some(mtu) = self.mtu {
            flags.mtu = mtu;
        }

        if let Some(limit) = self.instance_recv_bps_limit {
            flags.instance_recv_bps_limit = limit;
        }

        // Legacy flags override
        if let Some(v) = self.flags.get("mtu") {
            if let Ok(mtu) = v.parse::<u32>() {
                flags.mtu = mtu;
            }
        }
        if let Some(ref s) = self.flags.get("dev_name") {
            if !s.is_empty() {
                flags.dev_name = s.to_string();
            }
        }
        if let Some(ref algo) = self.flags.get("encryption_algorithm") {
            flags.encryption_algorithm = algo.to_string();
        }
        if let Some(ref s) = self.flags.get("default_protocol") {
            if !s.is_empty() {
                flags.default_protocol = s.to_string();
            }
        }
        if let Some(v) = self.flags.get("enable_encryption") {
            flags.enable_encryption = v == "true";
        }
        if let Some(v) = self.flags.get("enable_ipv6") {
            flags.enable_ipv6 = v == "true";
        }

        cfg.set_flags(flags);

        Ok(cfg)
    }
}
