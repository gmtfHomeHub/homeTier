use super::PlatformAdapter;
use crate::types::AuthResult;
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

    fn authorize_tun(&self) -> AuthResult {
        AuthResult { success: false, message: "iOS 暂不支持 TUN 模式".into(), needs_restart: false }
    }
}
