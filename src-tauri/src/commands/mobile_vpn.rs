// Mobile VPN status command - placeholder (Tauri 2 cannot invoke native plugins from Rust)
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VpnStatus {
    pub running: bool,
    pub ipv4_addr: Option<String>,
    pub routes: Vec<String>,
    pub dns: Option<String>,
}

#[tauri::command]
pub async fn get_vpn_status(
    _space_id: String,
) -> Result<VpnStatus, String> {
    // Tauri 2 不支持从 Rust 侧调用原生插件命令（无 AppHandle::invoke）。
    // 前端通过 `plugin:hometiervpnservice|get_vpn_status` 直接调用 Kotlin/Swift 插件。
    // 此命令作为占位符保留，返回默认状态。
    Ok(VpnStatus {
        running: false,
        ipv4_addr: None,
        routes: vec![],
        dns: None,
    })
}