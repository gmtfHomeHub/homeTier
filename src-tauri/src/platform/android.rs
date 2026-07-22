use super::PlatformAdapter;
use crate::types::AuthResult;
use std::path::PathBuf;

pub struct AndroidAdapter;

impl PlatformAdapter for AndroidAdapter {
    fn get_config_dir(&self) -> PathBuf {
        PathBuf::from("/data/data/com.hometier.app/files/config")
    }

    fn get_log_dir(&self) -> PathBuf {
        PathBuf::from("/data/data/com.hometier.app/cache/logs")
    }

    fn is_elevated(&self) -> bool {
        true // Android 不需要提权
    }

    fn get_platform_name(&self) -> &'static str {
        "android"
    }

    fn authorize_tun(&self) -> AuthResult {
        AuthResult { success: false, message: "Android 暂不支持 TUN 模式".into(), needs_restart: false }
    }
}
