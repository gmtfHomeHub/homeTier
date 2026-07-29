/// 退出时清理所有遗留文件
pub fn shutdown_exit_cleanup(data_dir: &std::path::Path) {
    crate::log_info!("[退出] 开始清理...");

    // 1. 清理 signal / state 文件
    remove_file_if_exists(&data_dir.join("daemon_ready.signal"));
    remove_file_if_exists(&data_dir.join("daemon_state.json"));

    // 2. 清理临时文件（easytier dl / extract）
    cleanup_temp_files();

    // 3. macOS: 关闭 easytier-core root 守护进程
    cleanup_root_daemon();

    crate::log_info!("[退出] 清理完成");
}

/// 启动前仅清理临时文件（不再干预任何进程）
pub fn startup_precheck(_toml_dir: &std::path::Path) {
    cleanup_temp_files();
}

fn remove_file_if_exists(path: &std::path::Path) {
    if path.exists() {
        if std::fs::remove_file(path).is_ok() {
            crate::log_info!(format!("[退出] 已移除 {}", path.display()));
        }
    }
}

/// macOS: 通过 RPC Shutdown 优雅关闭 easytier-core root daemon（防止端口泄露）
fn cleanup_root_daemon() {
    #[cfg(target_os = "macos")]
    {
        let root_rpc_port = crate::daemon::ipc::EASYTIER_DAEMON_RPC_PORT;
        let addr = format!("127.0.0.1:{}", root_rpc_port);
        match std::net::TcpStream::connect_timeout(
            &addr.parse().unwrap(),
            std::time::Duration::from_millis(500),
        ) {
            Ok(mut stream) => {
                crate::log_info!("[退出] 发送 Shutdown 到 easytier-core root daemon");
                let _ = std::io::Write::write_all(
                    &mut stream,
                    b"shutdown",
                );
            }
            Err(_) => {
                // root daemon 未运行，无需清理
            }
        }
    }
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