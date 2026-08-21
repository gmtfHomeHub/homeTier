// Mobile VPN integration tests
//
// These tests verify the mobile VPN flow works correctly
// Run with: cargo test --test mobile_vpn

use home_tier_lib::mobile::{TunConfig, build_vpn_config, TunProvider};
use home_tier_lib::easytier::{EasyTierManager, launcher::NetworkInstance};
use home_tier_lib::easytier::config::Config as EasyTierConfig;
use std::sync::Arc;
use std::path::PathBuf;
use uuid::Uuid;
use tokio::sync::Mutex;

// Mock TunProvider for testing
struct MockTunProvider {
    prepare_called: std::sync::atomic::AtomicBool,
    start_called: std::sync::atomic::AtomicBool,
    stop_called: std::sync::atomic::AtomicBool,
    fd_to_return: Arc<Mutex<Option<i32>>>,
}

impl MockTunProvider {
    fn new() -> Self {
        Self {
            prepare_called: std::sync::atomic::AtomicBool::new(false),
            start_called: std::sync::atomic::AtomicBool::new(false),
            stop_called: std::sync::atomic::AtomicBool::new(false),
            fd_to_return: Arc::new(Mutex::new(None)),
        }
    }

    async fn set_fd(&self, fd: i32) {
        *self.fd_to_return.lock().await = Some(fd);
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[async_trait::async_trait]
impl TunProvider for MockTunProvider {
    async fn prepare(&self) -> Result<(), String> {
        self.prepare_called.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn start_and_await_fd(
        &self,
        _space_id: Uuid,
        _ipv4_addr: &str,
        _routes: &[String],
        _mtu: u32,
        _excluded_app: Option<&str>,
    ) -> Result<i32, String> {
        self.start_called.store(true, std::sync::atomic::Ordering::SeqCst);
        
        // Wait for fd to be set
        for _ in 0..100 {
            if let Some(fd) = *self.fd_to_return.lock().await {
                return Ok(fd);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        Err("Timeout waiting for fd".into())
    }

    async fn stop(&self, _space_id: Uuid) -> Result<(), String> {
        self.stop_called.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn is_active(&self, _space_id: &Uuid) -> bool {
        false
    }
}

#[tokio::test]
#[cfg(any(target_os = "android", target_os = "ios"))]
async fn test_tun_provider_prepare() {
    let provider = MockTunProvider::new();
    let result = provider.prepare().await;
    assert!(result.is_ok());
    assert!(provider.prepare_called.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
#[cfg(any(target_os = "android", target_os = "ios"))]
async fn test_tun_provider_start_and_await_fd() {
    let provider = MockTunProvider::new();
    let space_id = Uuid::new_v4();
    
    // Set fd after a short delay
    let provider_clone = provider.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        provider_clone.set_fd(42).await;
    });

    let result = provider.start_and_await_fd(
        space_id,
        "10.144.144.1/24",
        &["10.144.144.0/24".to_string()],
        1500,
        Some("com.hometier.app"),
    ).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
    assert!(provider.start_called.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
#[cfg(any(target_os = "android", target_os = "ios"))]
async fn test_tun_provider_stop() {
    let provider = MockTunProvider::new();
    let space_id = Uuid::new_v4();
    
    let result = provider.stop(space_id).await;
    assert!(result.is_ok());
    assert!(provider.stop_called.load(std::sync::atomic::Ordering::SeqCst));
}

// Tests for the VPN config builder (runs on all platforms)
#[test]
fn test_tun_config_default() {
    let config = TunConfig::default();
    assert_eq!(config.virtual_ip, "10.144.144.1");
    assert_eq!(config.virtual_ip_cidr, 24);
    assert_eq!(config.mtu, 1500);
    assert_eq!(config.excluded_apps, vec!["com.hometier.app".to_string()]);
}

#[test]
fn test_build_vpn_config() {
    let mut config = TunConfig::default();
    config.space_id = Uuid::new_v4();
    config.network_name = "test_network".to_string();
    config.virtual_ip = "10.0.0.1".to_string();
    config.virtual_ip_cidr = 24;
    config.mtu = 1400;
    config.routes = vec!["10.0.0.0/24".to_string(), "192.168.1.0/24".to_string()];
    config.excluded_apps = vec!["com.test.app".to_string()];
    config.dns_servers = vec!["10.0.0.1".to_string(), "8.8.8.8".to_string()];

    let vpn_config = build_vpn_config(&config);
    
    assert!(vpn_config.interface_name.starts_with("tun_"));
    assert_eq!(vpn_config.virtual_ip, "10.0.0.1");
    assert_eq!(vpn_config.virtual_ip_cidr, 24);
    assert_eq!(vpn_config.mtu, 1400);
    assert_eq!(vpn_config.routes.len(), 2);
    assert_eq!(vpn_config.excluded_apps, vec!["com.test.app".to_string()]);
    assert_eq!(vpn_config.dns_servers.len(), 2);
}

// Tests for EasyTier network instance
#[tokio::test]
async fn test_network_instance_creation() {
    // Test that we can create a NetworkInstance with a basic config
    let config = EasyTierConfig::default();
    
    // This should not panic
    let instance = NetworkInstance::new(config).await;
    assert!(instance.is_ok());
    
    let instance = instance.unwrap();
    // Test that get_tun_fd_sender is available
    let sender = instance.get_tun_fd_sender();
    assert!(sender.is_some());
}

// Test EasyTierManager
#[tokio::test]
async fn test_easytier_manager_creation() {
    let temp_dir = std::env::temp_dir().join("hometier_test_manager");
    std::fs::create_dir_all(&temp_dir).ok();
    
    let easytier = EasyTierManager::new(
        temp_dir.clone(),
        temp_dir.clone(),
        None,
    );
    
    // The instances map should be empty initially
    assert!(easytier.list_running().is_empty());
}