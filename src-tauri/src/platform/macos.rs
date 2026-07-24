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
        // macOS utun 不需要 root 或额外授权，对所有用户可用
        AuthResult { success: true, message: "macOS utun 无需额外授权".into(), needs_restart: false }
    }
}
