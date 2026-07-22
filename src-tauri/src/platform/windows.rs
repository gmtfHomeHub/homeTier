use super::PlatformAdapter;
use crate::types::AuthResult;
use std::path::PathBuf;

pub struct WindowsAdapter;

impl PlatformAdapter for WindowsAdapter {
    fn get_config_dir(&self) -> PathBuf {
        directories::BaseDirs::new()
            .map(|d| d.config_dir().join("homeTier"))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn get_log_dir(&self) -> PathBuf {
        self.get_config_dir().join("logs")
    }

    fn is_elevated(&self) -> bool {
        unsafe {
            let mut elevated: u32 = 0;
            let mut token: *mut std::ffi::c_void = std::ptr::null_mut();
            let current_process = windows::Win32::System::Threading::GetCurrentProcess();
            if windows::Win32::Security::OpenProcessToken(
                current_process,
                windows::Win32::Security::TOKEN_QUERY,
                &mut token,
            )
            .as_bool()
            {
                let mut size: u32 = 0;
                let mut elevation = windows::Win32::Security::TOKEN_ELEVATION::default();
                windows::Win32::Security::GetTokenInformation(
                    token,
                    windows::Win32::Security::TokenElevation,
                    Some(&mut elevation as *mut _ as *mut std::ffi::c_void),
                    std::mem::size_of::<windows::Win32::Security::TOKEN_ELEVATION>() as u32,
                    &mut size,
                );
                elevation.TokenIsElevated != 0
            } else {
                false
            }
        }
    }

    fn get_platform_name(&self) -> &'static str {
        "windows"
    }

    fn authorize_tun(&self) -> AuthResult {
        // Windows TUN 需要管理员权限。通过 UAC 以 runas 重新启动自身。
        let exe = match std::env::current_exe() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(e) => {
                log_error!(format!("获取可执行路径失败: {}", e));
                crate::log::log_system("authorize_tun", &format!("获取可执行路径失败: {}", e));
                return AuthResult { success: false, message: "获取可执行路径失败".into(), needs_restart: false };
            }
        };

        log_error!(format!("Windows TUN 授权: 通过 runas 重启进程, exe={}", exe));
        crate::log::log_system("authorize_tun", &format!("Windows TUN 授权: 通过 runas 重启进程, exe={}", exe));

        // 使用 ShellExecuteW 以管理员身份启动新进程
        let result = unsafe {
            windows::Win32::UI::Shell::ShellExecuteW(
                None,
                windows::core::w!("runas"),
                &windows::core::HSTRING::from(&exe),
                windows::core::w!(""),
                None,
                windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            )
        };

        // ShellExecuteW 返回值 > 32 表示成功
        if result.0 > 32 {
            // 当前进程退出，新进程将以管理员身份运行
            std::process::exit(0);
        }

        AuthResult { success: false, message: "UAC 授权失败或已被取消".into(), needs_restart: false }
    }
}
