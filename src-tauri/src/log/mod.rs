use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::OnceLock;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[cfg(target_os = "windows")]
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringA;

/// 日志级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// 单条日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub seq: u64,
    pub timestamp: String,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub space_id: Option<String>,
}

/// 日志条数上限（超出时移除最早的记录）
const MAX_LOG_ENTRIES: usize = 5000;

/// 日志开关（默认开启；关闭时后端不再记录日志）
static LOG_ENABLED: AtomicBool = AtomicBool::new(true);

static LOG_STORE: OnceLock<Mutex<VecDeque<LogEntry>>> = OnceLock::new();
static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);

/// 转发通道：GUI 进程的 log_info! 将日志发送到 daemon
static FORWARD_TX: OnceLock<std::sync::mpsc::Sender<LogEntry>> = OnceLock::new();

/// 初始化日志转发（GUI 进程使用），所有 log_info! 将同步转发到 daemon
pub fn init_forward(tx: std::sync::mpsc::Sender<LogEntry>) {
    let _ = FORWARD_TX.set(tx);
}

fn store() -> &'static Mutex<VecDeque<LogEntry>> {
    LOG_STORE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// 设置日志开关
pub fn set_log_enabled(enabled: bool) {
    LOG_ENABLED.store(enabled, Ordering::Relaxed);
}

/// 查询日志开关
pub fn is_log_enabled() -> bool {
    LOG_ENABLED.load(Ordering::Relaxed)
}

/// 清空所有日志（应用退出时调用）
pub fn clear() {
    if let Ok(mut logs) = store().lock() {
        logs.clear();
    }
}

/// 初始化文件日志系统（将日志同时输出到磁盘文件）
pub fn init_file_logging(log_dir: &std::path::Path) {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::fmt::layer;
    use std::fs::File;

    let _ = std::fs::create_dir_all(log_dir);
    let log_file = log_dir.join("hometier.log");

    let file = match File::create(&log_file) {
        Ok(f) => f,
        Err(_) => return,
    };

    let file_layer = layer()
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .with_target(true);

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .try_init();
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
    if !LOG_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    let entry = LogEntry {
        seq,
        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        level,
        module: module.to_string(),
        message,
        space_id,
    };
    if let Ok(mut logs) = store().lock() {
        logs.push_back(entry.clone());
        while logs.len() > MAX_LOG_ENTRIES {
            logs.pop_front();
        }
    }
    // 转发到 daemon（仅 GUI 进程）
    if let Some(tx) = FORWARD_TX.get() {
        let _ = tx.send(entry);
    }
}

/// 获取所有日志（可选按级别过滤）
pub fn get_all(level_filter: Option<LogLevel>) -> Vec<LogEntry> {
    if let Ok(logs) = store().lock() {
        match level_filter {
            Some(lv) => logs.iter().filter(|e| e.level == lv).cloned().collect(),
            None => logs.iter().cloned().collect(),
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
    ($msg:expr, $space:expr) => { $crate::log::log($crate::log::LogLevel::Info, module_path!(), $msg.into(), Some(format!("{}", $space))) };
}

#[macro_export]
macro_rules! log_error {
    ($msg:expr) => { $crate::log::log($crate::log::LogLevel::Error, module_path!(), $msg.into(), None) };
    ($msg:expr, $space:expr) => { $crate::log::log($crate::log::LogLevel::Error, module_path!(), $msg.into(), Some(format!("{}", $space))) };
}

#[macro_export]
macro_rules! log_warn {
    ($msg:expr) => { $crate::log::log($crate::log::LogLevel::Warning, module_path!(), $msg.into(), None) };
    ($msg:expr, $space:expr) => { $crate::log::log($crate::log::LogLevel::Warning, module_path!(), $msg.into(), Some(format!("{}", $space))) };
}

#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => { $crate::log::log($crate::log::LogLevel::Debug, module_path!(), $msg.into(), None) };
    ($msg:expr, $space:expr) => { $crate::log::log($crate::log::LogLevel::Debug, module_path!(), $msg.into(), Some(format!("{}", $space))) };
}