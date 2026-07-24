use super::PlatformAdapter;
use crate::types::AuthResult;
use std::path::PathBuf;

pub struct MacOSAdapter;

impl PlatformAdapter for MacOSAdapter {
    fn get_config_dir(&self) -> PathBuf {
        directories::BaseDirs::new()
            .map(|d| d.config_dir().join("homeTier"))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn get_log_dir(&self) -> PathBuf {
        directories::BaseDirs::new()
            .map(|d| d.home_dir().join("Library").join("Logs").join("homeTier"))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn is_elevated(&self) -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    fn get_platform_name(&self) -> &'static str {
        "macos"
    }

    fn authorize_tun(&self) -> AuthResult {
        // macOS utun socket 创建不需要 root，但 ifconfig/route 网络配置需要 root
        // 检测当前是否有 root 权限
        if self.is_elevated() {
            AuthResult { success: true, message: "macOS 管理员权限已就绪".into(), needs_restart: false }
        } else {
            AuthResult {
                success: false,
                message: "macOS 配置虚拟网卡需要管理员权限。请使用守护进程模式:\n\
                    sudo hometier --daemon\n\
                    或者以管理员身份运行: sudo hometier".into(),
                needs_restart: false,
            }
        }
    }
}
