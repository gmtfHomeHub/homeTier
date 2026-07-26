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
        AuthResult {
            success: false,
            message: "iOS TUN 模式需要在Xcode中配置NetworkExtension权限。请添加Packet Tunnel Provider target并申请Network Extension entitlement。".into(),
            needs_restart: false,
        }
    }
}
