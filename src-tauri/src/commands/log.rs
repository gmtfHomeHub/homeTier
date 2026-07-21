use crate::log::{self, LogLevel};

#[tauri::command]
pub fn get_logs(level: Option<String>) -> Vec<log::LogEntry> {
    let level_filter = level.and_then(|l| match l.to_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    });
    log::get_all(level_filter)
}

#[tauri::command]
pub fn get_space_logs(space_id: String, level: Option<String>) -> Vec<log::LogEntry> {
    let level_filter = level.and_then(|l| match l.to_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    });
    log::get_by_space(&space_id, level_filter)
}

#[tauri::command]
pub fn clear_logs() {
    log::clear();
}