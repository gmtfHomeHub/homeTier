use super::PlatformAdapter;
use std::path::PathBuf;

pub struct IOSAdapter;

impl PlatformAdapter for IOSAdapter {
    fn get_config_dir(&self) -> PathBuf {
        PathBuf::from("/var/mobile/Documents/homeTier/config")
    }

    fn get_log_dir(&self) -> PathBuf {
        PathBuf::from("/var/mobile/Documents/homeTier/logs")
    }

    fn is_elevated(&self) -> bool {
        true // iOS 不需要提权
    }

    fn get_platform_name(&self) -> &'static str {
        "ios"
    }
}
