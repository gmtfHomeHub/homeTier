#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::path::PathBuf;

/// 平台适配器 trait
pub trait PlatformAdapter: Send + Sync {
    fn get_config_dir(&self) -> PathBuf;
    fn get_log_dir(&self) -> PathBuf;
    fn is_elevated(&self) -> bool;
    fn get_platform_name(&self) -> &'static str;
}

/// 获取平台适配器实例
pub fn get_adapter() -> Box<dyn PlatformAdapter> {
    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsAdapter);
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOSAdapter);
    #[cfg(target_os = "android")]
    return Box::new(android::AndroidAdapter);
    #[cfg(target_os = "ios")]
    return Box::new(ios::IOSAdapter);
}

/// 获取平台特定数据目录
pub fn get_data_dir() -> PathBuf {
    let adapter = get_adapter();
    adapter.get_config_dir()
}

/// 获取平台特定日志目录
pub fn get_log_dir() -> PathBuf {
    let adapter = get_adapter();
    adapter.get_log_dir()
}
