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

/// 启动前清理：释放被旧 daemon 占用的端口 + 清理临时文件
pub fn startup_precheck(_toml_dir: &std::path::Path) {
    kill_any_daemon_occupying_port();
    cleanup_temp_files();
}

/// 连接 15889 → 发送 Shutdown → 轮询等待端口释放（最长 5s）
fn kill_any_daemon_occupying_port() {
    let addr = format!("127.0.0.1:{}", crate::daemon::ipc::DEFAULT_RPC_PORT);
    let sock_addr: std::net::SocketAddr = match addr.parse() {
        Ok(a) => a,
        Err(_) => return,
    };

    // 1. 连接旧 daemon
    let mut stream = match std::net::TcpStream::connect_timeout(
        &sock_addr,
        std::time::Duration::from_millis(500),
    ) {
        Ok(s) => {
            crate::log_info!("[启动] 发现残留 daemon 进程，发送关闭指令");
            s
        }
        Err(_) => return, // 无残留进程
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(2000)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(2000)));

    // 发送 Shutdown
    let shutdown_json = serde_json::json!({"type": "Shutdown"});
    if let Ok(json) = serde_json::to_string(&shutdown_json) {
        let len = json.len() as u32;
        use std::io::Write;
        let _ = stream.write_all(&len.to_le_bytes());
        let _ = stream.write_all(json.as_bytes());
    }

    // 2. 轮询端口是否已释放（最长 5 秒）
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if std::net::TcpStream::connect_timeout(&sock_addr, std::time::Duration::from_millis(200)).is_err() {
            crate::log_info!("[启动] 残留 daemon 已释放端口");
            return;
        }
    }
    crate::log_info!("[启动] 残留 daemon 端口未在 5 秒内释放，继续启动");
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