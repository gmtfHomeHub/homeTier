// iOS NetworkExtension VPN commands
use tauri::State;
use crate::types::NetworkConfig;
use crate::space::manager::SpaceManager;
use std::sync::Arc;
use uuid::Uuid;
use tauri::Manager;

#[tauri::command]
#[cfg(target_os = "ios")]
pub async fn start_ios_vpn(
    space_id: String,
    config_json: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("invalid space_id: {}", e))?;

    // Write VPN config to App Group shared file
    // The NEPacketTunnelProvider extension reads from this file
    let app_group_id = "group.com.hometier.app";
    
    #[cfg(target_os = "ios")]
    {
        // Get App Group container path
        // On iOS, the App Group container is at: /var/mobile/Containers/Shared/AppGroup/<group_id>/
        // Or we can use the directories crate
        use std::path::Path;
        
        // Try to find the App Group container
        let base_paths = [
            format!("/var/mobile/Containers/Shared/AppGroup/{}", app_group_id),
            format!("/private/var/mobile/Containers/Shared/AppGroup/{}", app_group_id),
        ];
        
        let mut config_written = false;
        for base_path in &base_paths {
            let prefs_dir = Path::new(base_path).join("Library/Preferences");
            if prefs_dir.exists() || std::fs::create_dir_all(&prefs_dir).is_ok() {
                let config_path = prefs_dir.join("VPNConfig.json");
                if std::fs::write(&config_path, &config_json).is_ok() {
                    config_written = true;
                    crate::log_info!(format!("iOS VPN config written to: {}", config_path.display()));
                    break;
                }
            }
        }
        
        if !config_written {
            // Fallback: try using the app's container with App Group entitlement
            if let Ok(home) = std::env::var("HOME") {
                let prefs_dir = Path::new(&home).join("Library/Preferences");
                if prefs_dir.exists() || std::fs::create_dir_all(&prefs_dir).is_ok() {
                    let config_path = prefs_dir.join("VPNConfig.json");
                    if std::fs::write(&config_path, &config_json).is_ok() {
                        config_written = true;
                        crate::log_info!(format!("iOS VPN config written to fallback: {}", config_path.display()));
                    }
                }
            }
        }
        
        if !config_written {
            crate::log_warn!("Failed to write iOS VPN config to any App Group location");
        }
    }

    // Emit event to start the NE tunnel via NETunnelProviderManager
    // This requires a Tauri plugin or Swift bridge to call NETunnelProviderManager
    // For now, we'll emit an event that can be handled by a Swift bridge
    let _ = app_handle.emit("ios:start-vpn", serde_json::json!({
        "spaceId": space_id.to_string(),
        "config": serde_json::Value::String(config_json)
    }));

    Ok(())
}

#[tauri::command]
#[cfg(target_os = "ios")]
pub async fn stop_ios_vpn(
    space_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("invalid space_id: {}", e))?;

    // Emit event to stop the NE tunnel
    let _ = app_handle.emit("ios:stop-vpn", serde_json::json!({
        "spaceId": space_id.to_string()
    }));

    // Also clear the config file
    let app_group_id = "group.com.hometier.app";
    #[cfg(target_os = "ios")]
    {
        let base_paths = [
            format!("/var/mobile/Containers/Shared/AppGroup/{}", app_group_id),
            format!("/private/var/mobile/Containers/Shared/AppGroup/{}", app_group_id),
        ];
        
        for base_path in &base_paths {
            let config_path = std::path::Path::new(base_path).join("Library/Preferences/VPNConfig.json");
            if config_path.exists() {
                let _ = std::fs::remove_file(&config_path);
            }
        }
    }

    Ok(())
}