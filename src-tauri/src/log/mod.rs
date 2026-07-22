use serde::Serialize;
use std::sync::OnceLock;
use std::sync::Mutex;

#[cfg(target_os = "windows")]
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringA;

/// 日志级别
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// 单条日志条目
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub space_id: Option<String>,
}

static LOG_STORE: OnceLock<Mutex<Vec<LogEntry>>> = OnceLock::new();

fn store() -> &'static Mutex<Vec<LogEntry>> {
    LOG_STORE.get_or_init(|| Mutex::new(Vec::new()))
}

/// 清空所有日志（应用退出时调用）
pub fn clear() {
    if let Ok(mut logs) = store().lock() {
        logs.clear();
    }
}

/// 将消息同时写入应用内存日志和 OS 系统日志。
/// 适用于授权失败等重要错误。
pub fn log_system(tag: &str, message: &str) {
    log(LogLevel::Error, tag, message.to_string(), None);
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("logger")
            .args(["-t", &format!("homeTier[{}]", tag), "-p", "user.err", "--", message])
            .status();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("logger")
            .args(["-t", &format!("homeTier[{}]", tag), "-p", "user.err", "--", message])
            .status();
    }
    #[cfg(target_os = "windows")]
    {
        // Windows Event Log 写入 (简化: 通过 OutputDebugString)
        let formatted = format!("homeTier[{}]: {}\0", tag, message);
        unsafe {
            OutputDebugStringA(
                windows::core::s!(formatted),
            );
        }
    }
}

/// 记录一条日志
pub fn log(level: LogLevel, module: &str, message: String, space_id: Option<String>) {
    let entry = LogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        level,
        module: module.to_string(),
        message,
        space_id,
    };
    if let Ok(mut logs) = store().lock() {
        logs.push(entry);
    }
}

/// 获取所有日志（可选按级别过滤）
pub fn get_all(level_filter: Option<LogLevel>) -> Vec<LogEntry> {
    if let Ok(logs) = store().lock() {
        match level_filter {
            Some(lv) => logs.iter().filter(|e| e.level == lv).cloned().collect(),
            None => logs.clone(),
        }
    } else {
        vec![]
    }
}

/// 获取指定空间的日志（可选按级别过滤）
pub fn get_by_space(space_id: &str, level_filter: Option<LogLevel>) -> Vec<LogEntry> {
    if let Ok(logs) = store().lock() {
        logs.iter()
            .filter(|e| e.space_id.as_deref() == Some(space_id))
            .filter(|e| match &level_filter {
                Some(lv) => e.level == *lv,
                None => true,
            })
            .cloned()
            .collect()
    } else {
        vec![]
    }
}

// ---- 便捷宏 ----

#[macro_export]
macro_rules! log_info {
    ($msg:expr) => { $crate::log::log($crate::log::LogLevel::Info, module_path!(), $msg.into(), None) };
    ($msg:expr, $space:expr) => { $crate::log::log($crate::log::LogLevel::Info, module_path!(), $msg.into(), Some($space.into())) };
}

#[macro_export]
macro_rules! log_error {
    ($msg:expr) => { $crate::log::log($crate::log::LogLevel::Error, module_path!(), $msg.into(), None) };
    ($msg:expr, $space:expr) => { $crate::log::log($crate::log::LogLevel::Error, module_path!(), $msg.into(), Some($space.into())) };
}

#[macro_export]
macro_rules! log_warn {
    ($msg:expr) => { $crate::log::log($crate::log::LogLevel::Warning, module_path!(), $msg.into(), None) };
    ($msg:expr, $space:expr) => { $crate::log::log($crate::log::LogLevel::Warning, module_path!(), $msg.into(), Some($space.into())) };
}

#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => { $crate::log::log($crate::log::LogLevel::Debug, module_path!(), $msg.into(), None) };
    ($msg:expr, $space:expr) => { $crate::log::log($crate::log::LogLevel::Debug, module_path!(), $msg.into(), Some($space.into())) };
}