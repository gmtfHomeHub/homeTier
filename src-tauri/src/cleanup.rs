/// 退出时的简单清理
pub fn shutdown_exit_cleanup() {
    crate::log_info!("[退出] 开始清理...");
    cleanup_temp_files();
    crate::log_info!("[退出] 清理完成");
}

/// 启动前仅清理临时文件（不再干预任何进程）
pub fn startup_precheck(_toml_dir: &std::path::Path) {
    cleanup_temp_files();
}

fn cleanup_temp_files() {
    let temp_dir = std::env::temp_dir();
    let mut removed = 0u32;

    if let Ok(entries) = std::fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if name.starts_with("easytier-dl-") || name.starts_with("easytier-extract-") {
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
                removed += 1;
            }
        }
    }

    if removed > 0 {
        crate::log_info!(format!("[Cleanup] 已清理 {} 个临时文件/目录", removed));
    }
}