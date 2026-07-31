/// 统一清理入口（启动前 + 退出时）
pub fn cleanup_all(easytier_config_dir: &std::path::Path) {
    // 清理临时文件
    cleanup_temp_files();

    // 关闭 easytier-core 守护进程（基于 PID 文件，防止残留）
    cleanup_easytier_daemon(easytier_config_dir);

    crate::log_info!("[Cleanup] 清理完成");
}

/// 基于 PID 文件优雅关闭 easytier-core 守护进程。
/// 仅当 PID 对应的进程名确为 easytier-core 时才终止，避免 PID 复用误杀其他进程。
pub(crate) fn cleanup_easytier_daemon(config_dir: &std::path::Path) {
    let pid_file = config_dir.join("easytier-core.pid");
    let Ok(content) = std::fs::read_to_string(&pid_file) else {
        return;
    };
    let Ok(pid) = content.trim().parse::<u32>() else {
        return;
    };

    // 进程名校验（防 PID 复用误杀）
    if !is_easytier_process(pid) {
        crate::log_warn!(format!(
            "[Cleanup] PID {} 不是 easytier-core 进程，跳过清理（残留 pid 文件将被移除）",
            pid
        ));
        let _ = std::fs::remove_file(&pid_file);
        return;
    }

    crate::log_info!(format!("[Cleanup] 关闭 easytier-core 守护进程, pid={}", pid));

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // 先 SIGTERM 优雅退出（检查返回值；EPERM 表示当前用户无权终止 root 进程）
        unsafe {
            if libc::kill(pid as i32, libc::SIGTERM) != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EPERM) {
                    crate::log_warn!(format!("[Cleanup] 发送 SIGTERM 无权限（EPERM），尝试提权终止, pid={}", pid));
                    #[cfg(target_os = "macos")]
                    escalate_kill(pid, libc::SIGTERM);
                } else {
                    crate::log_error!(format!("[Cleanup] 发送 SIGTERM 失败: {}", err));
                }
            }
        }

        // 等待退出（最多 5s）
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if !is_process_alive(pid) {
                break;
            }
        }

        // 仍存活则 SIGKILL 强杀
        if is_process_alive(pid) {
            crate::log_warn!(format!("[Cleanup] easytier-core 未在 5s 内退出，发送 SIGKILL, pid={}", pid));
            unsafe {
                if libc::kill(pid as i32, libc::SIGKILL) != 0 {
                    let err = std::io::Error::last_os_error();
                    if err.raw_os_error() == Some(libc::EPERM) {
                        crate::log_warn!(format!("[Cleanup] 发送 SIGKILL 无权限（EPERM），尝试提权强杀, pid={}", pid));
                        #[cfg(target_os = "macos")]
                        escalate_kill(pid, libc::SIGKILL);
                    } else {
                        crate::log_error!(format!("[Cleanup] 发送 SIGKILL 失败: {}", err));
                    }
                }
            }
            // 提权强杀后稍等，确认退出
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if !is_process_alive(pid) {
                    break;
                }
            }
        }
    }

    // 仅当进程确认已死亡时才移除 PID 文件；否则保留以便下次启动继续兜底
    if !is_process_alive(pid) {
        let _ = std::fs::remove_file(&pid_file);
    } else {
        crate::log_warn!(format!("[Cleanup] 进程仍存活，保留 PID 文件以在下次启动时继续清理, pid={}", pid));
    }
}

/// macOS 非 root 环境对 root 进程提权终止（osascript 管理员权限）。
/// 仅在常规 kill 因 EPERM 失败时作为兜底使用（正常路径 daemon 已以 root 身份完成清理）。
#[cfg(target_os = "macos")]
fn escalate_kill(pid: u32, signal: libc::c_int) {
    let sig = match signal {
        libc::SIGTERM => "SIGTERM",
        libc::SIGKILL => "SIGKILL",
        _ => "SIGTERM",
    };
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!(
            r#"do shell script "kill -{} {}" with administrator privileges"#,
            sig.trim_start_matches("SIG"),
            pid
        ))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn is_process_alive(pid: u32) -> bool {
    // kill(pid, 0): 0=存在, ESRCH=不存在, EPERM=存在但无权限(仍算存活)
    unsafe {
        let ret = libc::kill(pid as i32, 0);
        if ret == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn is_process_alive(_pid: u32) -> bool {
    false
}

/// 通过 ps 校验指定 PID 的进程名是否为 easytier-core
fn is_easytier_process(pid: u32) -> bool {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if let Ok(out) = std::process::Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("comm=")
            .output()
        {
            let name = String::from_utf8_lossy(&out.stdout);
            return name.trim().contains("easytier-core");
        }
        false
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        false
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
