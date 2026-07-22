use async_trait::async_trait;
use std::net::Ipv4Addr;

use super::{TunConfig, TunDeviceInfo, TunManager};
use tun_easytier::AbstractDevice;
use easytier::common::ifcfg::IfConfiguerTrait;

pub struct AndroidTunManager;

#[async_trait]
impl TunManager for AndroidTunManager {
    async fn create_tun(&self, _config: TunConfig) -> Result<TunDeviceInfo, String> {
        Err("Android 上 TUN 设备由 VpnService 创建，请使用 create_tun_from_fd".into())
    }

    async fn create_tun_from_fd(&self, fd: i32, config: TunConfig) -> Result<TunDeviceInfo, String> {
        let mut tun_cfg = tun_easytier::Configuration::default();
        tun_cfg
            .layer(tun_easytier::Layer::L3)
            .raw_fd(fd)
            .close_fd_on_drop(false)
            .up();
        let device = tun_easytier::create(&tun_cfg).map_err(|e| format!("TUN fd 创建失败: {}", e))?;
        let ifname = format!("tun{}", fd);

        let ifcfg = easytier::common::ifcfg::IfConfiger {};
        ifcfg.set_link_status(&ifname, true)
            .await
            .map_err(|e| format!("link up 失败: {}", e))?;

        if let Some(ip) = config.ip {
            let ip_addr: Ipv4Addr = ip.parse().map_err(|e| format!("无效 IP: {}", e))?;
            let prefix = config.cidr_prefix.unwrap_or(24);
            ifcfg.add_ipv4_ip(&ifname, ip_addr, prefix)
                .await
                .map_err(|e| format!("设置 IP 失败: {}", e))?;
        }

        if config.mtu > 0 {
            ifcfg.set_mtu(&ifname, config.mtu)
                .await
                .map_err(|e| format!("设置 MTU 失败: {}", e))?;
        }

        crate::log_info!(format!("Android TUN 已创建: fd={}, mtu={}", fd, config.mtu));

        Ok(TunDeviceInfo {
            name: ifname,
            ip: config.ip,
            mtu: config.mtu,
            platform: "android",
            fd: Some(fd),
        })
    }

    async fn destroy_tun(&self, name: &str) -> Result<(), String> {
        let ifcfg = easytier::common::ifcfg::IfConfiger {};
        ifcfg.set_link_status(name, false)
            .await
            .map_err(|e| format!("关闭 TUN {} 失败: {}", name, e))?;
        crate::log_info!(format!("TUN 已删除: {}", name));
        Ok(())
    }

    async fn set_link_status(&self, name: &str, up: bool) -> Result<(), String> {
        let ifcfg = easytier::common::ifcfg::IfConfiger {};
        ifcfg.set_link_status(name, up)
            .await
            .map_err(|e| format!("set_link_status {} 失败: {}", name, e))
    }

    async fn set_ip(&self, name: &str, ip: Ipv4Addr, prefix: u8) -> Result<(), String> {
        let ifcfg = easytier::common::ifcfg::IfConfiger {};
        ifcfg.add_ipv4_ip(name, ip, prefix)
            .await
            .map_err(|e| format!("set_ip {} 失败: {}", name, e))
    }

    async fn set_mtu(&self, name: &str, mtu: u32) -> Result<(), String> {
        let ifcfg = easytier::common::ifcfg::IfConfiger {};
        ifcfg.set_mtu(name, mtu)
            .await
            .map_err(|e| format!("set_mtu {} 失败: {}", name, e))
    }

    async fn add_route(&self, name: &str, dest: Ipv4Addr, mask: u8) -> Result<(), String> {
        let ifcfg = easytier::common::ifcfg::IfConfiger {};
        ifcfg.add_ipv4_route(name, dest, mask, None)
            .await
            .map_err(|e| format!("add_route {} 失败: {}", name, e))
    }

    async fn remove_route(&self, name: &str, dest: Ipv4Addr, mask: u8) -> Result<(), String> {
        let ifcfg = easytier::common::ifcfg::IfConfiger {};
        ifcfg.remove_ipv4_route(name, dest, mask)
            .await
            .map_err(|e| format!("remove_route {} 失败: {}", name, e))
    }
}
