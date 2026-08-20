//! iOS logger setup using os_log and App Group file logging

use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use directories::ProjectDirs;
use tracing::Level;
use tracing_subscriber::{fmt, EnvFilter};

static LOG_FILE: OnceLock<std::sync::Mutex<Option<std::fs::File>>> = OnceLock::new();

/// Setup iOS logger with os_log and optional file output
pub fn setup_ios_logger(path: &str, level: &str, subsystem: &str) -> std::io::Result<()> {
    // Parse log level
    let level = match level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    // Setup os_log (system log)
    let env_filter = EnvFilter::new(format!("{}={},easytier={}", subsystem, level, level));

    // Initialize tracing with os_log writer
    #[cfg(target_os = "ios")]
    {
        use tracing_oslog::OsLog;
        let os_log = OsLog::new(subsystem);
        fmt::Subscriber::builder()
            .with_env_filter(env_filter)
            .with_writer(move || os_log.clone())
            .init();
    }

    #[cfg(not(target_os = "ios"))]
    {
        fmt::Subscriber::builder()
            .with_env_filter(env_filter)
            .init();
    }

    // Also setup file logging to App Group container if path provided
    if !path.is_empty() {
        let log_path = PathBuf::from(path);
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        LOG_FILE.get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap()
            .replace(file);
    }

    Ok(())
}

/// Clear logger (close file handle)
pub fn clear_logger() {
    if let Some(mutex) = LOG_FILE.get() {
        mutex.lock().unwrap().take();
    }
}

/// Write a log line to the file (if enabled)
pub fn write_log_line(line: &str) {
    if let Some(mutex) = LOG_FILE.get() {
        if let Ok(mut guard) = mutex.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = writeln!(file, "{}", line);
            }
        }
    }
}