use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
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

/// 日志分类
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LogCategory {
    System,
    Network,
    WebRTC,
    Data,
    Proxy,
    Daemon,
    Space,
    Server,
}

impl std::fmt::Display for LogCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogCategory::System => "system",
            LogCategory::Network => "network",
            LogCategory::WebRTC => "webrtc",
            LogCategory::Data => "data",
            LogCategory::Proxy => "proxy",
            LogCategory::Daemon => "daemon",
            LogCategory::Space => "space",
            LogCategory::Server => "server",
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
    pub target: String,
    pub module: String,
    pub category: LogCategory,
    pub message: String,
    pub space_id: Option<String>,
    pub trace_id: Option<String>,
}

impl LogRecord {
    pub fn new(level: LogLevel, target: &str, message: String, space_id: Option<String>, trace_id: Option<String>) -> Self {
        let target_str = target.to_string();
        let module = extract_module(&target_str);
        let category = categorize_module(&module);
        Self {
            seq: 0,
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level,
            target: target_str,
            module,
            category,
            message,
            space_id,
            trace_id,
        }
    }

    pub fn module(&self) -> &str {
        &self.module
    }
}

/// 从 target 路径提取模块标签
fn extract_module(target: &str) -> String {
    // target 形如 "home_tier_lib::easytier::manager" 或 "crate::proxy::server"
    // 提取第一个有意义的模块名
    let parts: Vec<&str> = target.split("::").collect();
    for part in parts.iter().rev() {
        if !part.is_empty() && *part != "crate" {
            return part.to_string();
        }
    }
    parts.last().unwrap_or(&"unknown").to_string()
}

/// 根据模块名映射到分类
fn categorize_module(module: &str) -> LogCategory {
    match module {
        m if m.starts_with("easytier") || m.contains("network") || m.contains("space") => LogCategory::Network,
        m if m.contains("voice") || m.contains("screen") || m.contains("webrtc") => LogCategory::WebRTC,
        m if m.contains("chat") || m.contains("file") || m.contains("data") => LogCategory::Data,
        m if m.contains("proxy") => LogCategory::Proxy,
        m if m.contains("daemon") => LogCategory::Daemon,
        m if m.contains("space") => LogCategory::Space,
        m if m.contains("server") || m.contains("http") || m.contains("ws") => LogCategory::Server,
        _ => LogCategory::System,
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

/// 日志查询过滤器
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub level: Option<LogLevel>,
    pub space_id: Option<String>,
    pub module: Option<String>,
    pub category: Option<LogCategory>,
    pub keyword: Option<String>,
    pub since_seq: Option<u64>,
    /// 仅保留 timestamp 早于该 RFC3339 字符串（含）的日志
    pub before_ts: Option<String>,
    /// 仅保留 timestamp 晚于该 RFC3339 字符串（含）的日志
    pub after_ts: Option<String>,
    pub limit: Option<usize>,
}

impl LogFilter {
    pub fn matches(&self, record: &LogRecord) -> bool {
        if let Some(lv) = self.level {
            if record.level != lv {
                return false;
            }
        }
        if let Some(ref sid) = self.space_id {
            if record.space_id.as_deref() != Some(sid) {
                return false;
            }
        }
        if let Some(ref m) = self.module {
            if !record.module.contains(m) {
                return false;
            }
        }
        if let Some(cat) = self.category {
            if record.category != cat {
                return false;
            }
        }
        if let Some(ref kw) = self.keyword {
            let kw_lower = kw.to_lowercase();
            if !record.message.to_lowercase().contains(&kw_lower)
                && !record.target.to_lowercase().contains(&kw_lower)
            {
                return false;
            }
        }
        if let Some(seq) = self.since_seq {
            if record.seq <= seq {
                return false;
            }
        }
        if let Some(ref ts) = self.before_ts {
            if record.timestamp.as_str() > ts.as_str() {
                return false;
            }
        }
        if let Some(ref ts) = self.after_ts {
            if record.timestamp.as_str() < ts.as_str() {
                return false;
            }
        }
        true
    }
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

    /// v2 复合查询
    pub fn query(&self, filter: &LogFilter) -> Vec<LogRecord> {
        let logs = match self.store.lock() {
            Ok(l) => l,
            Err(_) => return vec![],
        };
        let mut result: Vec<LogRecord> = logs
            .iter()
            .filter(|e| filter.matches(e))
            .cloned()
            .collect();
        if let Some(limit) = filter.limit {
            result.truncate(limit);
        }
        result
    }

    /// 返回当前缓存中的所有活跃模块（去重）
    pub fn active_modules(&self) -> Vec<String> {
        if let Ok(logs) = self.store.lock() {
            let mut set: HashSet<String> = HashSet::new();
            for e in logs.iter() {
                set.insert(e.module.clone());
            }
            set.into_iter().collect()
        } else {
            vec![]
        }
    }

    /// v2 清除：按过滤器清除匹配项；filter 为空则清空全部
    pub fn clear_filtered(&self, filter: &LogFilter) {
        if let Ok(mut logs) = self.store.lock() {
            if filter.level.is_none()
                && filter.space_id.is_none()
                && filter.module.is_none()
                && filter.category.is_none()
                && filter.keyword.is_none()
            {
                logs.clear();
            } else {
                logs.retain(|e| !filter.matches(e));
            }
        }
    }

    pub fn clear(&self) {
        if let Ok(mut logs) = self.store.lock() {
            logs.clear();
        }
    }

    /// 启动恢复：注入历史记录（按 seq 排序追加），并续接 seq 计数
    pub fn restore(&self, mut records: Vec<LogRecord>) {
        if records.is_empty() {
            return;
        }
        records.sort_by_key(|r| r.seq);
        let max_seq = records.last().map(|r| r.seq).unwrap_or(0);
        if let Ok(mut logs) = self.store.lock() {
            for r in records {
                logs.push_back(r);
                while logs.len() > self.max_entries {
                    logs.pop_front();
                }
            }
        }
        self.seq.store(max_seq + 1, Ordering::Relaxed);
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

    /// 读取所有日志文件（含滚动文件），按 seq 排序，返回最近 n 条
    pub fn load_recent(&self, n: usize) -> Vec<LogRecord> {
        let mut paths: Vec<std::path::PathBuf> = vec![Path::new(&self.dir).join("hometier.log")];
        let mut i: u64 = 0;
        loop {
            let p = Path::new(&self.dir).join(format!("hometier.log.{}", i));
            if p.exists() {
                paths.push(p);
                i += 1;
            } else {
                break;
            }
        }

        let mut records: Vec<LogRecord> = Vec::new();
        for p in &paths {
            let Ok(content) = std::fs::read_to_string(p) else { continue };
            for line in content.lines() {
                if let Ok(r) = serde_json::from_str::<LogRecord>(line) {
                    records.push(r);
                }
            }
        }

        records.sort_by_key(|r| r.seq);
        if records.len() > n {
            records = records.split_off(records.len() - n);
        }
        records
    }

    /// 删除所有日志文件（含滚动文件），用于"清空日志"持久化语义
    pub fn delete_all(&self) {
        let mut paths: Vec<std::path::PathBuf> = vec![Path::new(&self.dir).join("hometier.log")];
        let mut i: u64 = 0;
        loop {
            let p = Path::new(&self.dir).join(format!("hometier.log.{}", i));
            if p.exists() {
                paths.push(p);
                i += 1;
            } else {
                break;
            }
        }
        for p in paths {
            let _ = std::fs::remove_file(p);
        }
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
            .add_backend(MemoryBackend::new(50000))
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
    let record = LogRecord::new(level, target, message, space_id, trace_id);
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
        backend.write(&LogRecord::new(LogLevel::Info, "test", "hello".into(), None, None));
        let logs = backend.get_all(None);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].seq, 1);
    }

    #[test]
    fn test_stdout_backend() {
        let backend = StdoutBackend::json();
        backend.write(&LogRecord::new(LogLevel::Error, "test", "error msg".into(), None, None));
    }
}
