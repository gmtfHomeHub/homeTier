use std::path::Path;

pub fn cleanup_all(_app_data_dir: &Path, easytier_config_dir: &Path) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    cleanup_stale_daemon();

    #[cfg(target_os = "macos")]
    cleanup_easytier_root();

    cleanup_temp_files();
    cleanup_toml_configs(easytier_config_dir);
    cleanup_orphan_easytier();
}

/// 应用退出时的最终清理（不检查端口、不需要重启 daemon）
pub fn shutdown_exit_cleanup() {
    crate::log_info!("[退出] 开始清理...");

    #[cfg(target_os = "macos")]
    {
        crate::log_info!("[退出] 关闭 macos easytier-core...");
        let addr = format!("127.0.0.1:{}", crate::daemon::ipc::EASYTIER_DAEMON_RPC_PORT);
        if let Ok(mut stream) = std::net::TcpStream::connect_timeout(
            &addr.parse().unwrap(),
            std::time::Duration::from_secs(3),
        ) {
            use std::io::Write;
            let _ = stream.write_all(b"__RPC_SHUTDOWN__\n");
            crate::log_info!("[退出] 已发送 shutdown 到 root easytier-core");
        }
    }

    cleanup_temp_files();
    crate::log_info!("[退出] 清理完成");
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn cleanup_stale_daemon() {
    use crate::daemon;

    let old_client = daemon::client::IpcClient::default_port();
    if old_client.ping_sync() {
        crate::log_info!("[Cleanup] 清理旧 daemon 进程...");
        old_client.shutdown_sync();
        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    #[cfg(unix)]
    if let Some((pid, _)) = daemon::ipc::load_daemon_state() {
        if daemon::ipc::is_process_alive(pid) {
            crate::log_info!(format!("[Cleanup] 强制终止旧 daemon 进程 pid={}", pid));
            unsafe { libc::kill(pid as i32, libc::SIGTERM); }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }

    daemon::ipc::clear_daemon_state();
    crate::log_info!("[Cleanup] daemon 状态文件已清除");
}

#[cfg(target_os = "macos")]
fn cleanup_easytier_root() {
    let addr = format!("127.0.0.1:{}", crate::daemon::ipc::EASYTIER_DAEMON_RPC_PORT);
    let name = "root easytier-core";
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(_) => return,
        };
        rt.block_on(async {
            use tokio::io::AsyncWriteExt;
            match tokio::net::TcpStream::connect(&addr).await {
                Ok(mut stream) => {
                    let _ = stream.writable().await;
                    let _ = stream.try_write(b"__RPC_SHUTDOWN__\n");
                    crate::log_info!(format!("[Cleanup] 已发送 shutdown 到 {} ({})", name, addr));
                }
                Err(_) => {
                    crate::log_info!(format!("[Cleanup] {} 未运行 ({})", name, addr));
                }
            }
        });
    });
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

    #[cfg(target_os = "macos")]
    {
        for path_str in ["/tmp/easytier-daemon-launch.sh", "/tmp/hometier-daemon.log", "/tmp/hometier-daemon.err"] {
            let p = std::path::Path::new(path_str);
            if p.exists() {
                let _ = std::fs::remove_file(p);
                removed += 1;
            }
        }
    }

    crate::log_info!(format!("[Cleanup] 已清理 {} 个临时文件/目录", removed));
}

fn cleanup_toml_configs(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut count = 0u32;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                let _ = std::fs::remove_file(&path);
                count += 1;
            }
        }
        if count > 0 {
            crate::log_info!(format!("[Cleanup] 已清理 {} 个残留 TOML 配置", count));
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
fn cleanup_orphan_easytier() {
    #[cfg(target_os = "linux")]
    {
        let result = std::process::Command::new("pkill")
            .args(["-f", "easytier-core.*--rpc-portal"])
            .output();
        if let Ok(output) = result {
            if output.status.success() {
                crate::log_info!("[Cleanup] 已清理孤儿 easytier-core 进程");
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", "easytier-core.exe", "/T"])
            .output();
    }
}

#[cfg(any(target_os = "android", target_os = "ios", target_os = "macos"))]
fn cleanup_orphan_easytier() {
    // macOS: cleanup_easytier_root() 已通过 RPC 关闭守护进程
    // Android/iOS: 无需清理
}
