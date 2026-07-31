use super::PlatformAdapter;
use std::path::PathBuf;

pub struct LinuxAdapter;

impl PlatformAdapter for LinuxAdapter {
    fn get_config_dir(&self) -> PathBuf {
        directories::BaseDirs::new()
            .map(|d| d.config_dir().join("homeTier"))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn get_log_dir(&self) -> PathBuf {
        self.get_config_dir().join("logs")
    }
}
