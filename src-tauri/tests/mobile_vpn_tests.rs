// Mobile VPN integration tests
//
// These tests verify the mobile VPN flow works correctly
// Run with: cargo test --test mobile_vpn

use home_tier_lib::easytier::{EasyTierManager, config::NetworkConfig};
use std::sync::Arc;
use std::path::PathBuf;
use uuid::Uuid;
use tokio::sync::Mutex;

// MockTunProvider and TunProvider tests removed - these were testing
// a mobile-specific API that doesn't exist in the current codebase.
// The mobile VPN implementation uses the Android VpnService/NetworkExtension
// directly via the Tauri plugin, not a Rust-level TunProvider abstraction.

// Tests for the VPN config (runs on all platforms)
// Note: TunConfig and build_vpn_config were from a planned mobile module
// that doesn't exist in the current implementation. The actual mobile VPN
// uses the Android VpnService/NetworkExtension via the Tauri plugin.

// Test EasyTierManager (public API)
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

// Test NetworkConfig default and serialization
#[test]
fn test_network_config_default() {
    let config = NetworkConfig::default();
    assert_eq!(config.dhcp, true);
    assert_eq!(config.network_name, "easytier");
    assert_eq!(config.network_secret, "");
    assert_eq!(config.networking_method, 1);
    assert_eq!(config.mtu, None);
    assert_eq!(config.virtual_ipv4, "");
}

#[test]
fn test_network_config_serialization() {
    let mut config = NetworkConfig::default();
    config.network_name = "test_network".to_string();
    config.network_secret = "test_secret".to_string();
    config.dhcp = true;
    config.virtual_ipv4 = "10.0.0.1".to_string();
    config.networking_method = 1;
    config.mtu = Some(1400);
    config.routes = vec!["10.0.0.0/24".to_string()];
    config.peer_urls = vec!["tcp://peer.example.com:11010".to_string()];
    
    // Test serialization round-trip
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: NetworkConfig = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.network_name, "test_network");
    assert_eq!(deserialized.network_secret, "test_secret");
    assert_eq!(deserialized.dhcp, true);
    assert_eq!(deserialized.virtual_ipv4, "10.0.0.1");
    assert_eq!(deserialized.networking_method, 1);
    assert_eq!(deserialized.mtu, Some(1400));
    assert_eq!(deserialized.routes.len(), 1);
    assert_eq!(deserialized.peer_urls.len(), 1);
}

// Test EasyTierManager network start/stop on mobile (Android/iOS only)
#[tokio::test]
#[cfg(any(target_os = "android", target_os = "ios"))]
async fn test_mobile_easytier_manager_start_stop() {
    let temp_dir = std::env::temp_dir().join("hometier_test_mobile");
    std::fs::create_dir_all(&temp_dir).ok();
    
    let easytier = EasyTierManager::new(
        temp_dir.clone(),
        temp_dir.clone(),
        None,
    );
    
    let instance_id = Uuid::new_v4();
    let mut config = NetworkConfig::default();
    config.network_name = "test".to_string();
    config.network_secret = "secret".to_string();
    config.dhcp = true;
    
    // This will fail in test environment without easytier-core, but tests the API
    let result = easytier.start_network(&config, instance_id, None).await;
    // We don't assert success because it requires easytier-core runtime
    // The test verifies the API is callable without panicking
    let _ = result;
    
    // Test stop
    let result = easytier.stop_network(&instance_id).await;
    let _ = result;
    
    assert!(easytier.list_running().is_empty());
}