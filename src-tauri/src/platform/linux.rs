use super::PlatformAdapter;
use crate::types::AuthResult;
use std::path::PathBuf;

pub struct LinuxAdapter;

impl LinuxAdapter {
    /// 通过 getcap 检查二进制文件是否具有 cap_net_admin 能力
    fn check_file_capabilities() -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let Some(exe_str) = exe.to_str() else {
            return false;
        };
        std::process::Command::new("getcap")
            .arg(exe_str)
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains("cap_net_admin")
            })
            .unwrap_or(false)
    }

    /// 获取手动设置能力的命令提示
    fn get_manual_setcap_command() -> String {
        let exe = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "<可执行文件路径>".into());
        format!("sudo setcap cap_net_admin+ep {}", exe)
    }
}

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

        // 1. 尝试 pkexec setcap
        match std::process::Command::new("pkexec")
            .args(["setcap", "cap_net_admin+ep", &exe])
            .status()
        {
            Ok(status) if status.success() => {
                log_system_auth("pkexec setcap 成功");
                return AuthResult { success: true, message: "授权成功，重启应用后虚拟网卡将可用".into(), needs_restart: true };
            }
            Ok(_) => {
                log_system_auth(&format!("pkexec setcap 被取消或失败, exe={}", exe));
            }
            Err(e) => {
                log_system_auth(&format!("pkexec 不可用 ({}), exe={}", e, exe));
            }
        }

        // 2. 尝试 sudo setcap
        match std::process::Command::new("sudo")
            .args(["setcap", "cap_net_admin+ep", &exe])
            .status()
        {
            Ok(status) if status.success() => {
                log_system_auth("sudo setcap 成功");
                return AuthResult { success: true, message: "授权成功，重启应用后虚拟网卡将可用".into(), needs_restart: true };
            }
            Ok(_) => {
                log_system_auth(&format!("sudo setcap 被取消或失败, exe={}", exe));
            }
            Err(e) => {
                log_system_auth(&format!("sudo 不可用 ({}), exe={}", e, exe));
            }
        }

        // 3. 检查文件是否已有能力（可能当前进程没有继承）
        if Self::check_file_capabilities() {
            log_system_auth("文件已有 cap_net_admin 能力，但当前进程未继承，建议重启");
            return AuthResult { success: false, message: "检测到文件已有授权，但需要重启应用才能生效".into(), needs_restart: true };
        }

        // 4. 提供手动设置命令
        let manual_cmd = Self::get_manual_setcap_command();
        let msg = format!(
            "自动授权不可用（pkexec/sudo 均失败）。\n请在终端中手动执行以下命令后重启应用：\n{}",
            manual_cmd
        );
        log_system_auth(&format!("所有自动授权方式失败, 手动命令: {}", manual_cmd));
        AuthResult { success: false, message: msg, needs_restart: false }
    }
}

fn log_system_auth(details: &str) {
    log_error!(details);
    // crate::log::log_system("authorize_tun", details);
}
