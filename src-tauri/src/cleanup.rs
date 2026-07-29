use std::path::Path;

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

/// 启动时 pre-check + 实用清理（清理临时文件 + 孤立配置 + 遗留 signal 文件）
/// 注意：不再清理 daemon 进程 — daemon 由自身 run() 中的端口冲突检查处理
pub fn startup_precheck(toml_dir: &Path) {
    #[cfg(target_os = "macos")]
    cleanup_easytier_root();

    cleanup_temp_files();
    cleanup_orphan_toml_configs(toml_dir);
    cleanup_orphan_easytier();
    cleanup_signal_file();
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

fn cleanup_orphan_toml_configs(dir: &Path) {
    let mut removed = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    removed.push(name.to_string());
                }
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    if !removed.is_empty() {
        crate::log_info!(format!(
            "[Cleanup] 已清理 {} 个残留 TOML 配置: {}",
            removed.len(),
            removed.join(", ")
        ));
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
fn cleanup_orphan_easytier() {
    #[cfg(target_os = "linux")]
    {
        crate::log_info!("[Cleanup] 检查孤儿 easytier-core 进程...");
        let result = std::process::Command::new("pkill")
            .args(["-f", "easytier-core.*--rpc-portal"])
            .output();
        if let Ok(output) = result {
            if output.status.success() {
                crate::log_info!("[Cleanup] 已清理孤儿 easytier-core 进程");
            } else {
                crate::log_info!("[Cleanup] 未发现孤儿 easytier-core 进程");
            }
        } else {
            crate::log_info!("[Cleanup] pkill 命令不可用，跳过孤儿进程清理");
        }
    }

    #[cfg(target_os = "windows")]
    {
        crate::log_info!("[Cleanup] 检查孤儿 easytier-core 进程...");
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

fn cleanup_signal_file() {
    let path = crate::daemon::ipc::get_signal_path();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
        crate::log_info!("[Cleanup] 已清除遗留 signal 文件");
    }
}
