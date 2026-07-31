use super::PlatformAdapter;
use std::path::PathBuf;

pub struct AndroidAdapter;

impl PlatformAdapter for AndroidAdapter {
    fn get_config_dir(&self) -> PathBuf {
        PathBuf::from("/data/data/com.hometier.app/files/config")
    }

    fn get_log_dir(&self) -> PathBuf {
        PathBuf::from("/data/data/com.hometier.app/cache/logs")
    }
}
