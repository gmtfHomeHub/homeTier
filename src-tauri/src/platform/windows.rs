use super::PlatformAdapter;
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
}
