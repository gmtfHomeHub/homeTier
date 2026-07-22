#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::types::AuthResult;

/// 缓存 TUN 可用性检查结果（启动时初始化）
static TUN_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// 平台适配器 trait
pub trait PlatformAdapter: Send + Sync {
    fn get_config_dir(&self) -> PathBuf;
    fn get_log_dir(&self) -> PathBuf;
    fn is_elevated(&self) -> bool;
    fn get_platform_name(&self) -> &'static str;

    /// 尝试获取 TUN 设备创建权限。
    /// 弹框由系统级授权工具处理（polkit / UAC / osascript）。
    /// 失败时详细错误写入系统日志。
    fn authorize_tun(&self) -> AuthResult;
}

/// 获取平台适配器实例
pub fn get_adapter() -> Box<dyn PlatformAdapter> {
    #[cfg(target_os = "linux")]
    return Box::new(linux::LinuxAdapter);
    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsAdapter);
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOSAdapter);
    #[cfg(target_os = "android")]
    return Box::new(android::AndroidAdapter);
    #[cfg(target_os = "ios")]
    return Box::new(ios::IOSAdapter);
}

/// 获取平台特定数据目录
pub fn get_data_dir() -> PathBuf {
    let adapter = get_adapter();
    adapter.get_config_dir()
}

/// 获取平台特定日志目录
pub fn get_log_dir() -> PathBuf {
    let adapter = get_adapter();
    adapter.get_log_dir()
}

/// 在应用启动时初始化 TUN 能力检查。
/// 对于 Linux 检查 CAP_NET_ADMIN，其他平台检查 is_elevated。
pub fn init_tun_cap_check() {
    let available = check_tun_available_inner();
    TUN_AVAILABLE.store(available, Ordering::SeqCst);
    log_info!(format!("TUN 能力检查完成: available={}", available));
}

/// 返回缓存的 TUN 可用性状态
pub fn is_tun_available() -> bool {
    TUN_AVAILABLE.load(Ordering::SeqCst)
}

/// 执行实际的 TUN 可用性检查（不缓存）
pub fn check_tun_available() -> bool {
    check_tun_available_inner()
}

fn check_tun_available_inner() -> bool {
    let adapter = get_adapter();

    #[cfg(target_os = "linux")]
    {
        // Linux: 检查 CAP_NET_ADMIN 能力位
        if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
            for line in content.lines() {
                if line.starts_with("CapEff:") {
                    if let Some(hex) = line.split_whitespace().nth(1) {
                        if let Ok(caps) = u64::from_str_radix(hex, 16) {
                            // CAP_NET_ADMIN = cap 12
                            return (caps & (1 << 12)) != 0;
                        }
                    }
                }
            }
        }
        false
    }

    #[cfg(not(target_os = "linux"))]
    {
        adapter.is_elevated()
    }
}
