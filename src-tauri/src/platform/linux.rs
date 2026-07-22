use super::PlatformAdapter;
use crate::types::AuthResult;
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

    fn is_elevated(&self) -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    fn get_platform_name(&self) -> &'static str {
        "linux"
    }

    fn authorize_tun(&self) -> AuthResult {
        let exe = match std::env::current_exe() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(e) => {
                log_system_auth(&format!("获取可执行路径失败: {}", e));
                return AuthResult { success: false, message: "获取可执行路径失败".into(), needs_restart: false };
            }
        };

        match std::process::Command::new("pkexec")
            .args(["setcap", "cap_net_admin+ep", &exe])
            .status()
        {
            Ok(status) if status.success() => {
                log_system_auth("pkexec setcap 成功");
                AuthResult { success: true, message: "授权成功，重启应用后虚拟网卡将可用".into(), needs_restart: true }
            }
            Ok(_) => {
                log_system_auth(&format!("pkexec setcap 被取消或失败, exe={}", exe));
                AuthResult { success: false, message: "授权被取消".into(), needs_restart: false }
            }
            Err(e) => {
                log_system_auth(&format!("pkexec 不可用 ({}), 尝试 sudo, exe={}", e, exe));
                match std::process::Command::new("sudo")
                    .args(["setcap", "cap_net_admin+ep", &exe])
                    .status()
                {
                    Ok(status) if status.success() => {
                        log_system_auth("sudo setcap 成功");
                        AuthResult { success: true, message: "授权成功，重启应用后虚拟网卡将可用".into(), needs_restart: true }
                    }
                    Ok(_) => {
                        log_system_auth(&format!("sudo setcap 被取消或失败, exe={}", exe));
                        AuthResult { success: false, message: "授权被取消".into(), needs_restart: false }
                    }
                    Err(e2) => {
                        log_system_auth(&format!("sudo 也不可用 ({}), exe={}", e2, exe));
                        AuthResult { success: false, message: "系统授权工具不可用（未安装 pkexec 或 sudo）".into(), needs_restart: false }
                    }
                }
            }
        }
    }
}

fn log_system_auth(details: &str) {
    log_error!(details);
    crate::log::log_system("authorize_tun", details);
}
