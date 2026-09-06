// Mobile VPN status command - queries the Kotlin/Swift VPN service process
use tauri::{AppHandle, Manager};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VpnStatus {
    pub running: bool,
    pub ipv4_addr: Option<String>,
    pub routes: Vec<String>,
    pub dns: Option<String>,
}

#[tauri::command]
pub async fn get_vpn_status(
    space_id: String,
    app_handle: AppHandle,
) -> Result<VpnStatus, String> {
    // Invoke the plugin command from Rust (Tauri 2 supports this)
    // This calls the Kotlin plugin's get_vpn_status on Android
    // On iOS, it would call the equivalent (if implemented)
    let result: Result<VpnStatus, _> = app_handle
        .invoke("plugin:hometiervpnservice|get_vpn_status", serde_json::json!({ "spaceId": space_id }))
        .await;

    match result {
        Ok(status) => Ok(status),
        Err(e) => {
            // On non-mobile or if plugin not available, return default
            crate::log_warn!(format!("get_vpn_status failed: {}", e));
            Ok(VpnStatus {
                running: false,
                ipv4_addr: None,
                routes: vec![],
                dns: None,
            })
        }
    }
}