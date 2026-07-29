/// 统一清理入口（启动前 + 退出时）
pub fn cleanup_all() {
    // 清理临时文件
    cleanup_temp_files();

    // macOS: 关闭 easytier-core root 守护进程
    #[cfg(target_os = "macos")]
    cleanup_root_daemon();

    crate::log_info!("[Cleanup] 清理完成");
}

/// macOS: 通过 RPC Shutdown 优雅关闭 easytier-core root daemon
#[cfg(target_os = "macos")]
fn cleanup_root_daemon() {
    let root_rpc_port = crate::daemon::ipc::EASYTIER_DAEMON_RPC_PORT;
    let addr = format!("127.0.0.1:{}", root_rpc_port);
    match std::net::TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        std::time::Duration::from_millis(500),
    ) {
        Ok(mut stream) => {
            crate::log_info!("[Cleanup] 发送 Shutdown 到 easytier-core root daemon");
            let _ = std::io::Write::write_all(&mut stream, b"shutdown");
        }
        Err(_) => {}
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