use super::PlatformAdapter;
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
}
