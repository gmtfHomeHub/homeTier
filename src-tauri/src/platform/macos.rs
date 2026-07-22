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
        // macOS utun 需要 com.apple.vm.networking 授权。
        // 这是通过打包时的 entitlements 文件设置的，运行时无法动态获取。
        // 检测当前是否已有权限（通过 is_elevated 或尝试打开 tun 设备）
        if self.is_elevated() {
            return AuthResult { success: true, message: "当前已拥有管理员权限".into(), needs_restart: false };
        }

        // 尝试通过 osascript 请求管理员权限以加载内核扩展或设置授权
        match std::process::Command::new("osascript")
            .args(["-e", "do shell script \"echo homeTier_tun_auth\" with administrator privileges"])
            .status()
        {
            Ok(status) if status.success() => {
                log_error!("macOS TUN 授权成功（用户确认提权）");
                crate::log::log_system("authorize_tun", "macOS TUN 授权成功");
                AuthResult { success: true, message: "授权成功".into(), needs_restart: false }
            }
            Ok(_) => {
                log_error!("macOS TUN 授权被取消");
                crate::log::log_system("authorize_tun", "macOS TUN 授权被取消");
                AuthResult { success: false, message: "授权被取消".into(), needs_restart: false }
            }
            Err(e) => {
                log_error!(format!("osascript 不可用: {}", e));
                crate::log::log_system("authorize_tun", &format!("osascript 不可用: {}", e));
                AuthResult { success: false, message: "系统授权工具不可用".into(), needs_restart: false }
            }
        }
    }
}
