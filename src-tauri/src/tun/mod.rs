use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "ios")]
mod ios;

/// TUN 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunDeviceInfo {
    pub name: String,
    pub ip: Option<String>,
    pub mtu: u32,
    pub platform: &'static str,
    pub fd: Option<i32>,
}

/// TUN 配置参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunConfig {
    pub dev_name: Option<String>,
    pub ip: Option<String>,
    pub cidr_prefix: Option<u8>,
    pub mtu: u32,
    pub routes: Vec<String>,
    pub persist: bool,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            dev_name: None,
            ip: None,
            cidr_prefix: None,
            mtu: 1380,
            routes: Vec::new(),
            persist: false,
        }
    }
}

/// TUN 管理器 trait — 封装 tun-easytier + IfConfiger
#[async_trait]
pub trait TunManager: Send + Sync {
    /// 创建 TUN 设备（桌面端）
    async fn create_tun(&self, config: TunConfig) -> Result<TunDeviceInfo, String>;

    /// 从外部 fd 创建 TUN 设备（移动端：Android VpnService / iOS NEPacketTunnelProvider）
    async fn create_tun_from_fd(&self, fd: i32, config: TunConfig) -> Result<TunDeviceInfo, String>;

    /// 删除 TUN 设备
    async fn destroy_tun(&self, name: &str) -> Result<(), String>;

    /// 设置网卡状态（up/down）
    async fn set_link_status(&self, name: &str, up: bool) -> Result<(), String>;

    /// 设置 IP 地址
    async fn set_ip(&self, name: &str, ip: Ipv4Addr, prefix: u8) -> Result<(), String>;

    /// 设置 MTU
    async fn set_mtu(&self, name: &str, mtu: u32) -> Result<(), String>;

    /// 添加路由
    async fn add_route(&self, name: &str, dest: Ipv4Addr, mask: u8) -> Result<(), String>;

    /// 删除路由
    async fn remove_route(&self, name: &str, dest: Ipv4Addr, mask: u8) -> Result<(), String>;
}

/// 获取平台对应的 TunManager 实例
pub fn get_tun_manager() -> Box<dyn TunManager> {
    #[cfg(target_os = "linux")]
    return Box::new(linux::LinuxTunManager);
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacosTunManager);
    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsTunManager);
    #[cfg(target_os = "android")]
    return Box::new(android::AndroidTunManager);
    #[cfg(target_os = "ios")]
    return Box::new(ios::IosTunManager);
}
