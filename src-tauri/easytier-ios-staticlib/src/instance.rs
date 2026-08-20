//! Network instance wrapper for iOS

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use easytier::{
    config::Config as EasyTierConfig,
    launcher::NetworkInstance,
    Error as EasyTierError,
};
use parking_lot::RwLock;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::error::{Error, Result};

/// Wrapper around NetworkInstance with callbacks
pub struct NetworkInstanceWrapper {
    instance: Arc<NetworkInstance>,
    stop_callback: Arc<Mutex<Option<extern "C" fn()>>>,
    running_info_callback: Arc<Mutex<Option<extern "C" fn()>>>,
    running_info_tx: mpsc::UnboundedSender<Value>,
}

impl NetworkInstanceWrapper {
    /// Create a new network instance from EasyTier config
    pub async fn new(config: EasyTierConfig) -> Result<Self> {
        let instance = NetworkInstance::new(config)
            .await
            .map_err(|e| Error::Instance(format!("Failed to create NetworkInstance: {}", e)))?;

        let instance = Arc::new(instance);
        let (running_info_tx, mut running_info_rx) = mpsc::unbounded_channel();

        let stop_callback = Arc::new(Mutex::new(None));
        let running_info_callback = Arc::new(Mutex::new(None));

        // Start the instance
        instance.start().await.map_err(|e| {
            Error::Instance(format!("Failed to start NetworkInstance: {}", e))
        })?;

        // Spawn task to poll instance status and emit running info
        let instance_clone = instance.clone();
        let running_info_tx_clone = running_info_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                if let Ok(status) = instance_clone.get_status().await {
                    let info = json!({
                        "status": format!("{:?}", status),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    if running_info_tx_clone.send(info).is_err() {
                        break; // Receiver dropped
                    }
                } else {
                    break; // Instance stopped
                }
            }
        });

        // Spawn task to handle running info callback
        let running_info_callback_clone = running_info_callback.clone();
        tokio::spawn(async move {
            while let Some(info) = running_info_rx.recv().await {
                if let Some(cb) = *running_info_callback_clone.lock().unwrap() {
                    cb();
                }
            }
        });

        Ok(Self {
            instance,
            stop_callback,
            running_info_callback,
            running_info_tx,
        })
    }

    /// Inject the TUN file descriptor
    pub async fn set_tun_fd(&self, fd: i32) -> Result<()> {
        let sender = self.instance.get_tun_fd_sender().ok_or_else(|| {
            Error::Instance("tun fd sender unavailable".to_string())
        })?;

        sender.try_send(Some(fd)).map_err(|e| {
            Error::Instance(format!("Failed to send tun fd: {}", e))
        })?;

        info!("TUN fd {} injected successfully", fd);
        Ok(())
    }

    /// Register stop callback
    pub fn register_stop_callback(&self, cb: Option<extern "C" fn()>) {
        *self.stop_callback.lock().unwrap() = cb;
    }

    /// Register running info callback
    pub fn register_running_info_callback(&self, cb: Option<extern "C" fn()>) {
        *self.running_info_callback.lock().unwrap() = cb;
    }

    /// Get current running info as JSON string
    pub fn get_running_info(&self) -> Result<String> {
        // This is a simplified version - in practice you'd get actual status
        let info = json!({
            "status": "running",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        Ok(info.to_string())
    }

    /// Stop the network instance
    pub async fn stop(&self) {
        info!("Stopping network instance");
        // Trigger stop callback if registered
        if let Some(cb) = *self.stop_callback.lock().unwrap() {
            cb();
        }
        // The instance will be dropped when wrapper is dropped
    }
}

/// Convert JSON config to EasyTier Config
pub fn json_to_easytier_config(config: Value) -> Result<EasyTierConfig> {
    let mut config_builder = EasyTierConfig::default();

    // Network identity
    if let Some(network_name) = config.get("network_name").and_then(|v| v.as_str()) {
        config_builder.network_name = network_name.to_string();
    }
    if let Some(network_secret) = config.get("network_secret").and_then(|v| v.as_str()) {
        config_builder.network_secret = network_secret.to_string();
    }

    // Instance name
    if let Some(instance_id) = config.get("instance_id").and_then(|v| v.as_str()) {
        config_builder.instance_name = Some(instance_id.to_string());
    } else if let Some(instance_name) = config.get("instance_name").and_then(|v| v.as_str()) {
        config_builder.instance_name = Some(instance_name.to_string());
    }

    // Hostname
    if let Some(hostname) = config.get("hostname").and_then(|v| v.as_str()) {
        config_builder.hostname = Some(hostname.to_string());
    }

    // DHCP
    if let Some(dhcp) = config.get("dhcp").and_then(|v| v.as_bool()) {
        config_builder.dhcp = dhcp;
    }

    // IPv4
    if let Some(virtual_ip) = config.get("virtual_ip").and_then(|v| v.as_str()) {
        config_builder.virtual_ip = virtual_ip.parse().ok();
    }
    if let Some(ipv4) = config.get("ipv4").and_then(|v| v.as_str()) {
        config_builder.ipv4 = Some(ipv4.parse().ok().flatten());
    }
    if let Some(network_length) = config.get("network_length").and_then(|v| v.as_u64()) {
        config_builder.network_length = Some(network_length as u8);
    }

    // IPv6
    if let Some(ipv6) = config.get("ipv6").and_then(|v| v.as_str()) {
        config_builder.ipv6 = Some(ipv6.parse().ok().flatten());
    }
    if let Some(disable_ipv6) = config.get("disable_ipv6").and_then(|v| v.as_bool()) {
        config_builder.disable_ipv6 = disable_ipv6;
    }

    // Peer URLs
    if let Some(peer_urls) = config.get("peer_urls").and_then(|v| v.as_array()) {
        let peers: Vec<_> = peer_urls
            .iter()
            .filter_map(|u| u.as_str().and_then(|s| s.parse::<url::Url>().ok()))
            .map(|url| easytier::common::config::PeerConfig {
                uri: url,
                peer_public_key: None,
            })
            .collect();
        if !peers.is_empty() {
            config_builder.peers = peers;
        }
    }

    // Listener URLs
    if let Some(listener_urls) = config.get("listener_urls").and_then(|v| v.as_array()) {
        let listeners: Vec<_> = listener_urls
            .iter()
            .filter_map(|u| u.as_str().and_then(|s| s.parse::<url::Url>().ok()))
            .collect();
        if !listeners.is_empty() {
            config_builder.listeners = listeners;
        }
    }

    // Proxy CIDRs / proxy networks
    if let Some(proxy_cidrs) = config.get("proxy_cidrs").and_then(|v| v.as_array()) {
        for cidr_str in proxy_cidrs.iter().filter_map(|v| v.as_str()) {
            if let Ok(cidr) = cidr_str.parse::<cidr::Ipv4Cidr>() {
                let _ = config_builder.add_proxy_cidr(cidr, None);
            }
        }
    }

    // Routes
    if let Some(routes) = config.get("routes").and_then(|v| v.as_array()) {
        let route_list: Vec<_> = routes
            .iter()
            .filter_map(|r| r.as_str().and_then(|s| s.parse::<cidr::Ipv4Cidr>().ok()))
            .collect();
        if !route_list.is_empty() {
            config_builder.routes = Some(route_list);
        }
    }

    // Exit nodes
    if let Some(exit_nodes) = config.get("exit_nodes").and_then(|v| v.as_array()) {
        let nodes: Vec<_> = exit_nodes
            .iter()
            .filter_map(|n| n.as_str().and_then(|s| s.parse::<std::net::IpAddr>().ok()))
            .collect();
        if !nodes.is_empty() {
            config_builder.exit_nodes = nodes;
        }
    }

    // MTU
    if let Some(mtu) = config.get("mtu").and_then(|v| v.as_u64()) {
        config_builder.mtu = Some(mtu as u16);
    }

    // Interface name
    if let Some(interface_name) = config.get("interface_name").and_then(|v| v.as_str()) {
        config_builder.interface_name = Some(interface_name.to_string());
    }

    // Excluded apps (for mobile VPN)
    if let Some(excluded_apps) = config.get("excluded_apps").and_then(|v| v.as_array()) {
        config_builder.excluded_apps = excluded_apps
            .iter()
            .filter_map(|a| a.as_str().map(|s| s.to_string()))
            .collect();
    }

    // DNS servers
    if let Some(dns_servers) = config.get("dns_servers").and_then(|v| v.as_array()) {
        config_builder.dns_servers = dns_servers
            .iter()
            .filter_map(|d| d.as_str().and_then(|s| s.parse().ok()))
            .collect();
    }

    // Flags
    let mut flags = config_builder.get_flags();
    
    if let Some(latency_first) = config.get("latency_first").and_then(|v| v.as_bool()) {
        flags.latency_first = latency_first;
    }
    if let Some(enable_kcp_proxy) = config.get("enable_kcp_proxy").and_then(|v| v.as_bool()) {
        flags.enable_kcp_proxy = enable_kcp_proxy;
    }
    if let Some(enable_quic_proxy) = config.get("enable_quic_proxy").and_then(|v| v.as_bool()) {
        flags.enable_quic_proxy = enable_quic_proxy;
    }
    if let Some(disable_p2p) = config.get("disable_p2p").and_then(|v| v.as_bool()) {
        flags.disable_p2p = disable_p2p;
    }
    if let Some(bind_device) = config.get("bind_device").and_then(|v| v.as_bool()) {
        flags.bind_device = bind_device;
    }
    if let Some(no_tun) = config.get("no_tun").and_then(|v| v.as_bool()) {
        flags.no_tun = no_tun;
    }
    if let Some(multi_thread) = config.get("multi_thread").and_then(|v| v.as_bool()) {
        flags.multi_thread = multi_thread;
    }
    if let Some(enable_encryption) = config.get("enable_encryption").and_then(|v| v.as_bool()) {
        flags.enable_encryption = enable_encryption;
    }
    if let Some(dev_name) = config.get("dev_name").and_then(|v| v.as_str()) {
        if !dev_name.is_empty() {
            flags.dev_name = dev_name.to_string();
        }
    }
    if let Some(encryption_algorithm) = config.get("encryption_algorithm").and_then(|v| v.as_str()) {
        flags.encryption_algorithm = encryption_algorithm.to_string();
    }
    if let Some(default_protocol) = config.get("default_protocol").and_then(|v| v.as_str()) {
        if !default_protocol.is_empty() {
            flags.default_protocol = default_protocol.to_string();
        }
    }
    if let Some(enable_ipv6) = config.get("enable_ipv6").and_then(|v| v.as_bool()) {
        flags.enable_ipv6 = enable_ipv6;
    }

    config_builder.set_flags(flags);

    Ok(config_builder)
}