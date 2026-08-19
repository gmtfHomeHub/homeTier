//! Mobile platform VPN integration
//!
//! This module provides platform-specific TUN device management for mobile platforms.
//! Android uses VpnService to obtain a TUN fd, iOS uses NetworkExtension.

use std::os::fd::RawFd;
use std::path::PathBuf;
use uuid::Uuid;

/// Trait for platform-specific TUN device providers
pub trait TunProvider: Send + Sync {
    /// Request a TUN file descriptor for the given space.
    /// Returns the fd on success, or an error message.
    fn request_tun(&self, space_id: Uuid, config: &TunConfig) -> Result<RawFd, String>;

    /// Prepare the VPN service (request authorization if needed).
    /// Returns true if preparation succeeded or was already prepared.
    fn prepare(&self) -> Result<bool, String>;

    /// Stop the VPN service and release the fd.
    fn stop(&self) -> Result<(), String>;
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

/// Android-specific TUN provider (stub - actual implementation in Kotlin)
#[cfg(target_os = "android")]
mod android {
    use super::*;

    /// Android VpnService provider - communicates with Kotlin VpnService via JNI/Tauri events
    pub struct AndroidVpnProvider;

    impl AndroidVpnProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl TunProvider for AndroidVpnProvider {
        fn request_tun(&self, space_id: Uuid, config: &TunConfig) -> Result<RawFd, String> {
            // The actual fd is obtained via Kotlin VpnService and passed through
            // the set_tun_fd Tauri command. This stub is for compile-time only.
            Err("Android VpnProvider fd is obtained via Kotlin VpnService callback".into())
        }

        fn prepare(&self) -> Result<bool, String> {
            // VpnService.prepare() is called from Kotlin side
            Ok(true)
        }

        fn stop(&self) -> Result<(), String> {
            Ok(())
        }
    }
}

/// iOS-specific TUN provider (stub - actual implementation in Swift NetworkExtension)
#[cfg(target_os = "ios")]
mod ios {
    use super::*;

    /// iOS NetworkExtension provider - uses NEPacketTunnelProvider to obtain utun fd
    pub struct IosVpnProvider;

    impl IosVpnProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl TunProvider for IosVpnProvider {
        fn request_tun(&self, space_id: Uuid, config: &TunConfig) -> Result<RawFd, String> {
            // The actual fd is obtained via Swift NEPacketTunnelProvider and passed through
            // the set_tun_fd Tauri command. This stub is for compile-time only.
            Err("iOS VpnProvider fd is obtained via Swift NEPacketTunnelProvider callback".into())
        }

        fn prepare(&self) -> Result<bool, String> {
            Ok(true)
        }

        fn stop(&self) -> Result<(), String> {
            Ok(())
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

impl TunProvider for DesktopTunProvider {
    fn request_tun(&self, _space_id: Uuid, _config: &TunConfig) -> Result<RawFd, String> {
        Err("Desktop uses kernel TUN device directly, no fd injection needed".into())
    }

    fn prepare(&self) -> Result<bool, String> {
        Ok(true)
    }

    fn stop(&self) -> Result<(), String> {
        Ok(())
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