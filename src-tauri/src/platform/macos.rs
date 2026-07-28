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
        // Plan A: easytier-core 通过 osascript 直接提权启动，daemon 无需 root。
        // 此方法保留兼容旧调用链，始终返回 success。
        if self.is_elevated() {
            return AuthResult { success: true, message: "macOS 管理员权限已就绪".into(), needs_restart: false };
        }
        AuthResult {
            success: true,
            message: "macOS: easytier-core 将通过 osascript 提权启动".into(),
            needs_restart: false,
        }
    }
}
