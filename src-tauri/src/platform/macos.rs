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
        if self.is_elevated() {
            return AuthResult { success: true, message: "macOS 管理员权限已就绪".into(), needs_restart: false };
        }

        // 通过 osascript 弹出 macOS 原生授权对话框，以 root 权限重启 daemon
        let current_exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => return AuthResult {
                success: false,
                message: format!("无法获取当前可执行文件路径: {}", e),
                needs_restart: false,
            },
        };

        let exe_str = current_exe.to_string_lossy();
        let escaped = exe_str.replace("\\", "\\\\").replace("\"", "\\\"");
        let script = format!(
            "do shell script \"{} --daemon --elevated\" with administrator privileges",
            escaped
        );

        match std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    AuthResult {
                        success: true,
                        message: "macOS 管理员权限已获取，守护进程以 root 权限重启中...".into(),
                        needs_restart: true,
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    AuthResult {
                        success: false,
                        message: format!("授权失败: {}", stderr.trim()),
                        needs_restart: false,
                    }
                }
            }
            Err(e) => AuthResult {
                success: false,
                message: format!("启动 osascript 失败: {}", e),
                needs_restart: false,
            },
        }
    }
}
