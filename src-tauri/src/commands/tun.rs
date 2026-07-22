use crate::tun::{TunConfig, TunDeviceInfo, TunManager};

/// 创建 TUN 设备
#[tauri::command]
pub async fn create_tun(
    dev_name: Option<String>,
    ip: Option<String>,
    cidr_prefix: Option<u8>,
    mtu: Option<u32>,
    routes: Option<Vec<String>>,
) -> Result<TunDeviceInfo, String> {
    let manager = crate::tun::get_tun_manager();
    let config = TunConfig {
        dev_name,
        ip,
        cidr_prefix,
        mtu: mtu.unwrap_or(1380),
        routes: routes.unwrap_or_default(),
        persist: false,
    };
    manager.create_tun(config).await
}

/// 从外部 fd 创建 TUN 设备（Android VpnService / iOS NEPacketTunnelProvider）
#[tauri::command]
pub async fn create_tun_from_fd(
    fd: i32,
    ip: Option<String>,
    cidr_prefix: Option<u8>,
    mtu: Option<u32>,
) -> Result<TunDeviceInfo, String> {
    let manager = crate::tun::get_tun_manager();
    let config = TunConfig {
        dev_name: None,
        ip,
        cidr_prefix,
        mtu: mtu.unwrap_or(1380),
        routes: Vec::new(),
        persist: false,
    };
    manager.create_tun_from_fd(fd, config).await
}

/// 删除 TUN 设备
#[tauri::command]
pub async fn destroy_tun(name: String) -> Result<(), String> {
    let manager = crate::tun::get_tun_manager();
    manager.destroy_tun(&name).await
}

/// 设置 TUN 网卡状态（up/down）
#[tauri::command]
pub async fn set_tun_link_status(name: String, up: bool) -> Result<(), String> {
    let manager = crate::tun::get_tun_manager();
    manager.set_link_status(&name, up).await
}
