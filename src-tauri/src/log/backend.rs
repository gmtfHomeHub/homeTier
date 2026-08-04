use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};

// ---- 数据结构 ----

/// 日志级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warning => "warning",
            LogLevel::Error => "error",
        };
        write!(f, "{}", s)
    }
}

/// 统一日志记录结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub seq: u64,
    pub timestamp: String,
    pub level: LogLevel,
    #[serde(alias = "module")]
    pub target: String,
    pub message: String,
    pub space_id: Option<String>,
    pub trace_id: Option<String>,
    pub extra: Value,
}

impl LogRecord {
    pub fn new(level: LogLevel, target: &str, message: String) -> Self {
        Self {
            seq: 0,
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level,
            target: target.to_string(),
            message,
            space_id: None,
            trace_id: None,
            extra: Value::Null,
        }
    }

    pub fn module(&self) -> &str {
        &self.target
    }
}

// ---- 后端特征 ----

/// 后端特征
pub trait LogBackend: Send + Sync {
    fn write(&self, record: &LogRecord);
    fn flush(&self) {}
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn std::any::Any;
}

// ---- 内存后端 ----

/// 内存环形缓冲后端（桌面端）
pub struct MemoryBackend {
    store: Arc<Mutex<VecDeque<LogRecord>>>,
    max_entries: usize,
    seq: AtomicU64,
}

impl MemoryBackend {
    pub fn new(max_entries: usize) -> Self {
        Self {
            store: Arc::new(Mutex::new(VecDeque::with_capacity(max_entries))),
            max_entries,
            seq: AtomicU64::new(1),
        }
    }

    pub fn get_all(&self, level_filter: Option<LogLevel>) -> Vec<LogRecord> {
        if let Ok(logs) = self.store.lock() {
            match level_filter {
                Some(lv) => logs.iter().filter(|e| e.level == lv).cloned().collect(),
                None => logs.iter().cloned().collect(),
            }
        } else {
            vec![]
        }
    }

    pub fn get_by_space(&self, space_id: &str, level_filter: Option<LogLevel>) -> Vec<LogRecord> {
        if let Ok(logs) = self.store.lock() {
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

    pub fn clear(&self) {
        if let Ok(mut logs) = self.store.lock() {
            logs.clear();
        }
    }
}

impl LogBackend for MemoryBackend {
    fn write(&self, record: &LogRecord) {
        let mut entry = record.clone();
        entry.seq = self.seq.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut logs) = self.store.lock() {
            logs.push_back(entry);
            while logs.len() > self.max_entries {
                logs.pop_front();
            }
        }
    }

    fn name(&self) -> &'static str {
        "memory"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---- 文件后端 ----

/// 文件后端（直接写文件，按请求调 write，简化可控）
pub struct FileBackend {
    dir: String,
    max_size_bytes: u64,
    current_size: AtomicU64,
    seq: AtomicU64,
}

impl FileBackend {
    pub fn new(dir: &str, _retention_days: usize, max_size_mb: usize) -> Self {
        let _ = std::fs::create_dir_all(dir);
        let max_size_bytes = (max_size_mb as u64) * 1024 * 1024;
        let init_size = current_log_size(dir);
        Self {
            dir: dir.to_string(),
            max_size_bytes,
            current_size: AtomicU64::new(init_size),
            seq: AtomicU64::new(0),
        }
    }

    fn current_path(&self) -> String {
        Path::new(&self.dir).join("hometier.log").to_string_lossy().into_owned()
    }

    fn roll_path(&self) -> String {
        let n = self.seq.fetch_add(1, Ordering::Relaxed);
        Path::new(&self.dir).join(format!("hometier.log.{}", n)).to_string_lossy().into_owned()
    }
}

impl LogBackend for FileBackend {
    fn write(&self, record: &LogRecord) {
        let line = format!("{}\n", serde_json::to_string(record).unwrap_or_default());
        let size = line.len() as u64;
        let cur = self.current_size.load(Ordering::Relaxed);
        if cur + size > self.max_size_bytes {
            let _ = std::fs::rename(self.current_path(), self.roll_path());
            self.current_size.store(0, Ordering::Relaxed);
        }

        let path = self.current_path();
        let mut file = OpenOptions::new().create(true).append(true).open(&path);
        if let Ok(file) = file.as_mut() {
            let _ = file.write_all(line.as_bytes());
            self.current_size.fetch_add(size, Ordering::Relaxed);
        }
    }

    fn flush(&self) {
        // 每次写入已 flush append buffer
    }

    fn name(&self) -> &'static str {
        "file"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn current_log_size(dir: &str) -> u64 {
    let p = Path::new(dir).join("hometier.log");
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

// ---- Stdout 后端 ----

/// Stdout 后端（服务端容器友好）
pub struct StdoutBackend {
    json: bool,
}

impl StdoutBackend {
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    pub fn json() -> Self {
        Self::new(true)
    }
}

impl LogBackend for StdoutBackend {
    fn write(&self, record: &LogRecord) {
        if self.json {
            println!("{}", serde_json::to_string(record).unwrap_or_default());
        } else {
            println!(
                "{} [{}] {}: {}",
                record.timestamp, record.level, record.target, record.message
            );
        }
    }

    fn name(&self) -> &'static str {
        "stdout"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---- Syslog 后端 ----

/// Syslog 后端（可选，仅 Linux/macOS）
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub struct SyslogBackend {
    tag: String,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl SyslogBackend {
    pub fn new(tag: &str) -> Self {
        Self { tag: tag.to_string() }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl LogBackend for SyslogBackend {
    fn write(&self, record: &LogRecord) {
        use std::process::Command;
        let priority = match record.level {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warning => "warning",
            LogLevel::Error => "err",
        };
        let msg = format!("[{}] {}: {}", record.target, record.level, record.message);
        let _ = Command::new("logger")
            .args(["-t", &format!("homeTier[{}]", self.tag), "-p", &format!("user.{}", priority), "--", &msg])
            .status();
    }

    fn name(&self) -> &'static str {
        "syslog"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// 空实现（Windows 或未启用 syslog 时）
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub struct SyslogBackend;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl SyslogBackend {
    pub fn new(_tag: &str) -> Self {
        Self
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl LogBackend for SyslogBackend {
    fn write(&self, _record: &LogRecord) {}
    fn name(&self) -> &'static str {
        "syslog"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---- 转发后端 ----

/// 转发后端（桌面 GUI -> Daemon）
pub struct ForwardBackend {
    tx: OnceLock<std::sync::mpsc::Sender<LogRecord>>,
}

impl ForwardBackend {
    pub fn new() -> Self {
        Self { tx: OnceLock::new() }
    }

    pub fn init(&self, tx: std::sync::mpsc::Sender<LogRecord>) {
        let _ = self.tx.set(tx);
    }
}

impl LogBackend for ForwardBackend {
    fn write(&self, record: &LogRecord) {
        if let Some(tx) = self.tx.get() {
            let _ = tx.send(record.clone());
        }
    }

    fn name(&self) -> &'static str {
        "forward"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---- 分发器 ----

/// 分发器：聚合多个后端
pub struct Dispatch {
    pub backends: Vec<Box<dyn LogBackend>>,
}

impl Dispatch {
    pub fn new() -> Self {
        Self { backends: Vec::new() }
    }

    pub fn add_backend(mut self, backend: impl LogBackend + 'static) -> Self {
        self.backends.push(Box::new(backend));
        self
    }

    pub fn write(&self, record: &LogRecord) {
        for backend in &self.backends {
            backend.write(record);
        }
    }

    pub fn flush(&self) {
        for backend in &self.backends {
            backend.flush();
        }
    }
}

impl Default for Dispatch {
    fn default() -> Self {
        Self::new()
    }
}

// ---- 全局分发器 ----

/// 全局分发器
static GLOBAL_DISPATCH: OnceLock<Dispatch> = OnceLock::new();

/// 初始化全局分发器
pub fn init_dispatch(dispatch: Dispatch) {
    let _ = GLOBAL_DISPATCH.set(dispatch);
}

/// 获取全局分发器
pub fn global_dispatch() -> &'static Dispatch {
    GLOBAL_DISPATCH.get_or_init(|| {
        Dispatch::new()
            .add_backend(MemoryBackend::new(5000))
            .add_backend(ForwardBackend::new())
    })
}

/// 记录日志
pub fn dispatch_log(
    level: LogLevel,
    target: &str,
    message: String,
    space_id: Option<String>,
    trace_id: Option<String>,
) {
    let record = LogRecord {
        seq: 0,
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        level,
        target: target.to_string(),
        message,
        space_id,
        trace_id,
        extra: Value::Null,
    };
    global_dispatch().write(&record);
}

// ---- Trace ID 线程局部存储 ----

thread_local! {
    static CURRENT_TRACE_ID: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// 设置当前 trace_id
pub fn set_trace_id(trace_id: Option<String>) {
    CURRENT_TRACE_ID.with(|cell| *cell.borrow_mut() = trace_id);
}

/// 获取当前 trace_id
pub fn get_trace_id() -> Option<String> {
    CURRENT_TRACE_ID.with(|cell| cell.borrow().clone())
}

/// 便捷：从 HTTP 头提取 trace_id
pub fn extract_trace_id(headers: &http::HeaderMap, header_name: &str) -> Option<String> {
    headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

// ---- 测试 ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_backend() {
        let backend = MemoryBackend::new(10);
        backend.write(&LogRecord::new(LogLevel::Info, "test", "hello".into()));
        let logs = backend.get_all(None);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].seq, 1);
    }

    #[test]
    fn test_stdout_backend() {
        let backend = StdoutBackend::json();
        backend.write(&LogRecord::new(LogLevel::Error, "test", "error msg".into()));
    }
}
