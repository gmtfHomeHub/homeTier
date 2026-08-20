//! Mobile platform VPN integration
//!
//! This module provides platform-specific TUN device management for mobile platforms.
//! Android uses VpnService to obtain a TUN fd, iOS uses NetworkExtension.

use std::os::fd::RawFd;
use uuid::Uuid;

/// Trait for platform-specific TUN device providers
#[cfg(any(target_os = "android", target_os = "ios"))]
pub trait TunProvider: Send + Sync {
    /// Prepare the VPN service (request authorization if needed).
    /// Returns Ok(()) if preparation succeeded or was already prepared.
    async fn prepare(&self) -> Result<(), String>;

    /// Start VPN and block waiting for fd to be ready (timeout returns Err).
    ///
    /// - Android: triggers Kotlin VpnService → onStartCommand → establish → fd
    /// - iOS: triggers NE startTunnel → setTunnelNetworkSettings → fd
    async fn start_and_await_fd(
        &self,
        space_id: Uuid,
        ipv4_addr: &str,
        routes: &[String],
        mtu: u32,
        excluded_app: Option<&str>,
    ) -> Result<RawFd, String>;

    /// Stop VPN (clean up system VPN config + notify easytier)
    async fn stop(&self, space_id: Uuid) -> Result<(), String>;

    /// Health check
    fn is_active(&self, space_id: &Uuid) -> bool;
}

/// Configuration for TUN device creation
#[derive(Debug, Clone)]
pub struct TunConfig {
    pub space_id: Uuid,
    pub network_name: String,
    pub virtual_ip: String,
    pub virtual_ip_cidr: u8,
    pub mtu: u32,
    pub routes: Vec<String>,
    pub excluded_apps: Vec<String>,
    pub dns_servers: Vec<String>,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            space_id: Uuid::nil(),
            network_name: String::new(),
            virtual_ip: "10.144.144.1".to_string(),
            virtual_ip_cidr: 24,
            mtu: 1500,
            routes: vec!["10.144.144.0/24".to_string()],
            excluded_apps: vec!["com.hometier.app".to_string()],
            dns_servers: vec!["10.144.144.1".to_string()],
        }
    }
}

/// Android-specific TUN provider (actual fd obtained via Kotlin VpnService callback)
#[cfg(target_os = "android")]
mod android {
    use super::*;

    pub struct AndroidVpnProvider;

    impl AndroidVpnProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl TunProvider for AndroidVpnProvider {
        async fn prepare(&self) -> Result<(), String> {
            // VpnService.prepare() is called from Kotlin side via Tauri plugin
            Ok(())
        }

        async fn start_and_await_fd(
            &self,
            space_id: Uuid,
            ipv4_addr: &str,
            routes: &[String],
            mtu: u32,
            excluded_app: Option<&str>,
        ) -> Result<RawFd, String> {
            // The actual fd is obtained via Kotlin VpnService and passed through
            // the set_tun_fd Tauri command. This stub returns an error to indicate
            // the async fd wait pattern - the real flow is event-driven.
            Err("Android VpnProvider fd is obtained via Kotlin VpnService callback".into())
        }

        async fn stop(&self, space_id: Uuid) -> Result<(), String> {
            Ok(())
        }

        fn is_active(&self, space_id: &Uuid) -> bool {
            false // Not tracked in Rust; Kotlin VpnService manages lifecycle
        }
    }
}

/// iOS-specific TUN provider (actual fd obtained via Swift NEPacketTunnelProvider callback)
#[cfg(target_os = "ios")]
mod ios {
    use super::*;

    pub struct IosVpnProvider;

    impl IosVpnProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl TunProvider for IosVpnProvider {
        async fn prepare(&self) -> Result<(), String> {
            Ok(())
        }

        async fn start_and_await_fd(
            &self,
            space_id: Uuid,
            ipv4_addr: &str,
            routes: &[String],
            mtu: u32,
            excluded_app: Option<&str>,
        ) -> Result<RawFd, String> {
            // The actual fd is obtained via Swift NEPacketTunnelProvider and passed through
            // the set_tun_fd Tauri command. This stub returns an error to indicate
            // the async fd wait pattern - the real flow is event-driven.
            Err("iOS VpnProvider fd is obtained via Swift NEPacketTunnelProvider callback".into())
        }

        async fn stop(&self, space_id: Uuid) -> Result<(), String> {
            Ok(())
        }

        fn is_active(&self, space_id: &Uuid) -> bool {
            false // Not tracked in Rust; NE extension manages lifecycle
        }
    }
}

/// Get the platform-specific TUN provider
pub fn get_tun_provider() -> Box<dyn TunProvider> {
    #[cfg(target_os = "android")]
    {
        Box::new(android::AndroidVpnProvider::new())
    }
    #[cfg(target_os = "ios")]
    {
        Box::new(ios::IosVpnProvider::new())
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Box::new(DesktopTunProvider)
    }
}

/// Desktop stub provider (no VPN needed on desktop - uses system TUN)
struct DesktopTunProvider;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl TunProvider for DesktopTunProvider {
    async fn prepare(&self) -> Result<(), String> {
        Ok(())
    }

    async fn start_and_await_fd(
        &self,
        _space_id: Uuid,
        _ipv4_addr: &str,
        _routes: &[String],
        _mtu: u32,
        _excluded_app: Option<&str>,
    ) -> Result<RawFd, String> {
        Err("Desktop uses kernel TUN device directly, no fd injection needed".into())
    }

    async fn stop(&self, _space_id: Uuid) -> Result<(), String> {
        Ok(())
    }

    fn is_active(&self, _space_id: &Uuid) -> bool {
        false
    }
}

/// Generate the mobile VpnService configuration parameters
pub fn build_vpn_config(config: &TunConfig) -> VpnServiceConfig {
    VpnServiceConfig {
        interface_name: format!("tun_{}", config.space_id.simple().to_string()[..8].to_string()),
        virtual_ip: config.virtual_ip.clone(),
        virtual_ip_cidr: config.virtual_ip_cidr,
        mtu: config.mtu,
        routes: config.routes.clone(),
        excluded_apps: config.excluded_apps.clone(),
        dns_servers: config.dns_servers.clone(),
    }
}

/// Configuration passed to Android VpnService / iOS NEPacketTunnelProvider
#[derive(Debug, Clone)]
pub struct VpnServiceConfig {
    pub interface_name: String,
    pub virtual_ip: String,
    pub virtual_ip_cidr: u8,
    pub mtu: u32,
    pub routes: Vec<String>,
    pub excluded_apps: Vec<String>,
    pub dns_servers: Vec<String>,
}