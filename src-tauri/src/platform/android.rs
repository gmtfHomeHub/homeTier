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
        AuthResult {
            success: false,
            message: "Android TUN 模式需要系统级VPN权限。请在系统设置中开启VPN服务，当前版本暂未适配Android VPN框架。".into(),
            needs_restart: false,
        }
    }
}
