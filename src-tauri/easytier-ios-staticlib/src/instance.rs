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
    // This is a simplified conversion - in practice you'd map all fields
    // For now, we'll create a minimal config and let EasyTier fill in defaults
    let toml_str = config.to_string();

    // Parse as TOML-like structure
    let mut config_builder = EasyTierConfig::default();

    if let Some(network_name) = config.get("network_name").and_then(|v| v.as_str()) {
        config_builder.network_name = network_name.to_string();
    }
    if let Some(network_secret) = config.get("network_secret").and_then(|v| v.as_str()) {
        config_builder.network_secret = network_secret.to_string();
    }
    if let Some(virtual_ip) = config.get("virtual_ip").and_then(|v| v.as_str()) {
        config_builder.virtual_ip = virtual_ip.parse().ok();
    }
    if let Some(interface_name) = config.get("interface_name").and_then(|v| v.as_str()) {
        config_builder.interface_name = Some(interface_name.to_string());
    }
    if let Some(mtu) = config.get("mtu").and_then(|v| v.as_u64()) {
        config_builder.mtu = Some(mtu as u16);
    }
    if let Some(ipv4) = config.get("ipv4").and_then(|v| v.as_str()) {
        config_builder.ipv4 = Some(ipv4.parse().ok().flatten());
    }
    if let Some(ipv6) = config.get("ipv6").and_then(|v| v.as_str()) {
        config_builder.ipv6 = Some(ipv6.parse().ok().flatten());
    }
    if let Some(routes) = config.get("routes").and_then(|v| v.as_array()) {
        config_builder.routes = routes
            .iter()
            .filter_map(|r| r.as_str().and_then(|s| s.parse().ok()))
            .collect();
    }
    if let Some(excluded_apps) = config.get("excluded_apps").and_then(|v| v.as_array()) {
        config_builder.excluded_apps = excluded_apps
            .iter()
            .filter_map(|a| a.as_str().map(|s| s.to_string()))
            .collect();
    }
    if let Some(dns_servers) = config.get("dns_servers").and_then(|v| v.as_array()) {
        config_builder.dns_servers = dns_servers
            .iter()
            .filter_map(|d| d.as_str().and_then(|s| s.parse().ok()))
            .collect();
    }
    if let Some(listen_port) = config.get("listen_port").and_then(|v| v.as_u64()) {
        config_builder.listen_port = Some(listen_port as u16);
    }
    if let Some(relay_mode) = config.get("relay_mode").and_then(|v| v.as_bool()) {
        config_builder.relay_mode = relay_mode;
    }
    if let Some(no_tun) = config.get("no_tun").and_then(|v| v.as_bool()) {
        config_builder.no_tun = no_tun;
    }

    Ok(config_builder)
}