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
}
