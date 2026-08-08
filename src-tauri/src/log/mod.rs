pub mod backend;

use crate::log::backend::{
    Dispatch, FileBackend, ForwardBackend, MemoryBackend, StdoutBackend, SyslogBackend,
    dispatch_log, get_trace_id, global_dispatch, init_dispatch,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// 日志开关（默认开启；关闭时后端不再记录日志）
static LOG_ENABLED: AtomicBool = AtomicBool::new(true);

// ---- 对外公开的 re-exports ----

pub use crate::log::backend::{LogCategory, LogFilter, LogLevel, LogRecord};

/// 兼容别名（旧 LogEntry 类型，沿用约定）
pub type LogEntry = LogRecord;

// ---- 初始化 ----

/// 初始化日志系统
///
/// 根据环境变量 HOMETIER_MODE 自动选择后端组合：
/// - desktop (默认): 内存环形缓冲 + 转发 + 可选文件
/// - server: stdout(JSON) + 文件切割 + syslog
pub fn init_logger(mode: Option<&str>, log_dir: Option<&Path>, server_config: Option<&ServerLogConfig>) {
    let mode = mode.unwrap_or("desktop");

    let dispatch = match mode {
        "server" => build_server_dispatch(log_dir, server_config),
        _ => build_desktop_dispatch(log_dir),
    };

    init_dispatch(dispatch);
}

/// 服务端日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLogConfig {
    pub level: Option<String>,
    pub targets: Option<String>,
    pub file_dir: Option<String>,
    pub file_retention_days: Option<usize>,
    pub file_max_size_mb: Option<usize>,
    pub json_output: Option<bool>,
    pub trace_header: Option<String>,
}

impl Default for ServerLogConfig {
    fn default() -> Self {
        Self {
            level: None,
            targets: None,
            file_dir: None,
            file_retention_days: None,
            file_max_size_mb: None,
            json_output: None,
            trace_header: None,
        }
    }
}

fn build_desktop_dispatch(log_dir: Option<&Path>) -> Dispatch {
    let mut dispatch = Dispatch::new()
        .add_backend(MemoryBackend::new(50000))
        .add_backend(ForwardBackend::new());

    if let Some(dir) = log_dir {
        dispatch = dispatch.add_backend(FileBackend::new(
            dir.to_str().unwrap_or("./logs"),
            7,
            100,
        ));
    }

    dispatch
}

fn build_server_dispatch(_log_dir: Option<&Path>, config: Option<&ServerLogConfig>) -> Dispatch {
    let config = config.cloned().unwrap_or_default();
    let targets = config.targets.as_deref().unwrap_or("stdout,file,syslog");
    let file_dir = config.file_dir.as_deref().unwrap_or("/var/log/hometier");
    let retention_days = config.file_retention_days.unwrap_or(30);
    let max_size_mb = config.file_max_size_mb.unwrap_or(100);

    let mut dispatch = Dispatch::new();

    if targets.contains("stdout") {
        dispatch = dispatch.add_backend(StdoutBackend::json());
    }

    if targets.contains("file") {
        dispatch = dispatch.add_backend(FileBackend::new(file_dir, retention_days, max_size_mb));
    }

    if targets.contains("syslog") {
        dispatch = dispatch.add_backend(SyslogBackend::new("hometier"));
    }

    dispatch
}

// ---- 转发通道初始化 ----

/// 兼容旧 API：初始化转发通道
pub fn init_forward(tx: std::sync::mpsc::Sender<LogRecord>) {
    let dispatch = global_dispatch();
    for backend in &dispatch.backends {
        if let Some(forward) = backend.as_any().downcast_ref::<ForwardBackend>() {
            forward.init(tx);
            break;
        }
    }
}

// ---- 开关 ----

/// 设置日志开关
pub fn set_log_enabled(enabled: bool) {
    LOG_ENABLED.store(enabled, Ordering::Relaxed);
}

/// 查询日志开关
pub fn is_log_enabled() -> bool {
    LOG_ENABLED.load(Ordering::Relaxed)
}

// ---- 查询（仅桌面内存后端可用）----

/// 清空所有日志
pub fn clear() {
    let dispatch = global_dispatch();
    for backend in &dispatch.backends {
        if let Some(mem) = backend.as_any().downcast_ref::<MemoryBackend>() {
            mem.clear();
            break;
        }
    }
}

/// 获取所有日志
pub fn get_all(level_filter: Option<LogLevel>) -> Vec<LogRecord> {
    let dispatch = global_dispatch();
    for backend in &dispatch.backends {
        if let Some(mem) = backend.as_any().downcast_ref::<MemoryBackend>() {
            return mem.get_all(level_filter);
        }
    }
    vec![]
}

/// 获取指定空间的日志
pub fn get_by_space(space_id: &str, level_filter: Option<LogLevel>) -> Vec<LogRecord> {
    let dispatch = global_dispatch();
    for backend in &dispatch.backends {
        if let Some(mem) = backend.as_any().downcast_ref::<MemoryBackend>() {
            return mem.get_by_space(space_id, level_filter);
        }
    }
    vec![]
}

/// v2 复合查询
pub fn query(filter: &LogFilter) -> Vec<LogRecord> {
    let dispatch = global_dispatch();
    for backend in &dispatch.backends {
        if let Some(mem) = backend.as_any().downcast_ref::<MemoryBackend>() {
            return mem.query(filter);
        }
    }
    vec![]
}

/// v2 按过滤器清除
pub fn clear_filtered(filter: &LogFilter) {
    let dispatch = global_dispatch();
    for backend in &dispatch.backends {
        if let Some(mem) = backend.as_any().downcast_ref::<MemoryBackend>() {
            mem.clear_filtered(filter);
            break;
        }
    }
}

/// 当前活跃模块列表（供前端 UI 渲染模块筛选器）
pub fn active_modules() -> Vec<String> {
    let dispatch = global_dispatch();
    for backend in &dispatch.backends {
        if let Some(mem) = backend.as_any().downcast_ref::<MemoryBackend>() {
            return mem.active_modules();
        }
    }
    vec![]
}

// ---- 系统日志 ----

/// 系统日志写入（授权失败等重要错误）
pub fn log_system(tag: &str, message: &str) {
    crate::log_error!(message, tag);

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let _ = std::process::Command::new("logger")
            .args([
                "-t",
                &format!("homeTier[{}]", tag),
                "-p",
                "user.err",
                "--",
                message,
            ])
            .status();
    }
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Diagnostics::Debug::OutputDebugStringA;
        let formatted = format!("homeTier[{}]: {}\0", tag, message);
        unsafe {
            OutputDebugStringA(windows::core::s!(formatted));
        }
    }
}

// ---- 记录日志 ----

/// 记录一条日志
pub fn log(level: LogLevel, module: &str, message: String, space_id: Option<String>) {
    if !LOG_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    let trace_id = get_trace_id();
    dispatch_log(level, module, message, space_id, trace_id);
}

// ---- 桌面端 init_file_logging ----

/// 初始化文件日志（桌面可选调用）
pub fn init_file_logging(log_dir: &Path) {
    let _ = std::fs::create_dir_all(log_dir);
    let dispatch = global_dispatch();
    for backend in &dispatch.backends {
        if backend.name() == "file" {
            return;
        }
    }
    let new_dispatch = Dispatch::new()
        .add_backend(MemoryBackend::new(50000))
        .add_backend(ForwardBackend::new())
        .add_backend(FileBackend::new(
            log_dir.to_str().unwrap_or("./logs"),
            7,
            100,
        ));
    init_dispatch(new_dispatch);
}

// ---- 宏 ----

#[macro_export]
macro_rules! log_info {
    ($msg:expr) => {
        $crate::log::log(
            $crate::log::LogLevel::Info,
            module_path!(),
            $msg.into(),
            None,
        )
    };
    ($msg:expr, $space:expr) => {
        $crate::log::log(
            $crate::log::LogLevel::Info,
            module_path!(),
            $msg.into(),
            Some(format!("{}", $space)),
        )
    };
    ($msg:expr, $space:expr, trace_id: $trace:expr) => {
        $crate::log::dispatch_log(
            $crate::log::LogLevel::Info,
            module_path!(),
            $msg.into(),
            Some(format!("{}", $space)),
            Some($trace.into()),
        )
    };
}

#[macro_export]
macro_rules! log_error {
    ($msg:expr) => {
        $crate::log::log(
            $crate::log::LogLevel::Error,
            module_path!(),
            $msg.into(),
            None,
        )
    };
    ($msg:expr, $space:expr) => {
        $crate::log::log(
            $crate::log::LogLevel::Error,
            module_path!(),
            $msg.into(),
            Some(format!("{}", $space)),
        )
    };
    ($msg:expr, $space:expr, trace_id: $trace:expr) => {
        $crate::log::dispatch_log(
            $crate::log::LogLevel::Error,
            module_path!(),
            $msg.into(),
            Some(format!("{}", $space)),
            Some($trace.into()),
        )
    };
}

#[macro_export]
macro_rules! log_warn {
    ($msg:expr) => {
        $crate::log::log(
            $crate::log::LogLevel::Warning,
            module_path!(),
            $msg.into(),
            None,
        )
    };
    ($msg:expr, $space:expr) => {
        $crate::log::log(
            $crate::log::LogLevel::Warning,
            module_path!(),
            $msg.into(),
            Some(format!("{}", $space)),
        )
    };
    ($msg:expr, $space:expr, trace_id: $trace:expr) => {
        $crate::log::dispatch_log(
            $crate::log::LogLevel::Warning,
            module_path!(),
            $msg.into(),
            Some(format!("{}", $space)),
            Some($trace.into()),
        )
    };
}

#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => {
        $crate::log::log(
            $crate::log::LogLevel::Debug,
            module_path!(),
            $msg.into(),
            None,
        )
    };
    ($msg:expr, $space:expr) => {
        $crate::log::log(
            $crate::log::LogLevel::Debug,
            module_path!(),
            $msg.into(),
            Some(format!("{}", $space)),
        )
    };
    ($msg:expr, $space:expr, trace_id: $trace:expr) => {
        $crate::log::dispatch_log(
            $crate::log::LogLevel::Debug,
            module_path!(),
            $msg.into(),
            Some(format!("{}", $space)),
            Some($trace.into()),
        )
    };
}
