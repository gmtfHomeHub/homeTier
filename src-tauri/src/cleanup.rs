/// 统一清理入口（启动前 + 退出时）
pub fn cleanup_all(data_dir: &std::path::Path, easytier_config_dir: &std::path::Path) {
    // 清理临时文件
    cleanup_temp_files();

    // 清理残留的 homeTier daemon 进程（异常退出后兜底）
    cleanup_orphan_daemon(data_dir);

    // 关闭 easytier-core 守护进程（基于 PID 文件 + 端口兜底，防止残留）
    cleanup_easytier_daemon(easytier_config_dir);

    crate::log_info!("[Cleanup] 清理完成");
}

/// 清理残留的 homeTier daemon 进程（GUI 启动前 / 退出时兜底）。
/// 场景：GUI 异常退出后，root 权限的 daemon（--daemon 进程）存活。
/// 途径：daemon.pid 文件 + IPC 端口监听者双重检测，均需进程名校验防误杀（绝不清除 GUI 自身）。
pub(crate) fn cleanup_orphan_daemon(data_dir: &std::path::Path) {
    // 1. 基于 daemon.pid 文件
    let pid_file = data_dir.join("daemon.pid");
    if let Ok(content) = std::fs::read_to_string(&pid_file) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            if is_hometier_daemon_process(pid) {
                crate::log_info!(format!("[Cleanup] 关闭残留 daemon 进程, pid={}", pid));
                terminate_process(pid);
            } else {
                crate::log_warn!(format!(
                    "[Cleanup] PID {} 不是 homeTier daemon 进程，跳过（残留 pid 文件将被移除）",
                    pid
                ));
                let _ = std::fs::remove_file(&pid_file);
            }
        }
    }

    // 2. 端口兜底：IPC 端口被监听说明有 daemon（或异常残留）存在
    let ipc_port = crate::daemon::ipc::default_rpc_port();
    if let Some(pid) = find_listener_pid(ipc_port) {
        if is_hometier_daemon_process(pid) {
            crate::log_warn!(format!(
                "[Cleanup] 检测到 daemon 监听端口 {} 残留 (pid={})，清理",
                ipc_port, pid
            ));
            terminate_process(pid);
        } else {
            crate::log_warn!(format!(
                "[Cleanup] 端口 {} 被非 homeTier daemon 进程占用，跳过清理 (pid={})",
                ipc_port, pid
            ));
        }
    }
}

/// SIGTERM → 等待 5s → SIGKILL 终止指定进程；EPERM 时 macOS 走提权兜底。
/// 返回是否已确认进程终止。
fn terminate_process(pid: u32) -> bool {
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
                return true;
            }
        }

        // 仍存活则 SIGKILL 强杀
        crate::log_warn!(format!("[Cleanup] 进程未在 5s 内退出，发送 SIGKILL, pid={}", pid));
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
                return true;
            }
        }
        !is_process_alive(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        false
    }
}

/// 基于 PID 文件优雅关闭 easytier-core 守护进程；PID 文件缺失/失效时
/// 按 RPC 端口查找监听者兜底清理。
/// 仅当 PID 对应的进程名确为 easytier-core 时才终止，避免 PID 复用误杀其他进程。
pub(crate) fn cleanup_easytier_daemon(config_dir: &std::path::Path) {
    let pid_file = config_dir.join("easytier-core.pid");

    if let Ok(content) = std::fs::read_to_string(&pid_file) {
        if let Ok(pid) = content.trim().parse::<u32>() {
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

            // 仅当进程确认已死亡时才移除 PID 文件；否则保留以便下次启动继续兜底
            if terminate_process(pid) {
                let _ = std::fs::remove_file(&pid_file);
            } else {
                crate::log_warn!(format!("[Cleanup] 进程仍存活，保留 PID 文件以在下次启动时继续清理, pid={}", pid));
            }
            return;
        }
    }

    // 兜底：PID 文件缺失或已失效（如 daemon 被强杀未及写 PID 文件）时，
    // 按 easytier RPC 端口查找监听者并校验进程名后清理
    let rpc_port = crate::daemon::ipc::easytier_daemon_rpc_port();
    if let Some(pid) = find_listener_pid(rpc_port) {
        if is_easytier_process(pid) {
            crate::log_warn!(format!(
                "[Cleanup] 检测到 easytier-core 监听端口 {} 残留 (pid={})，清理",
                rpc_port, pid
            ));
            terminate_process(pid);
        }
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
            r#"do shell script "kill -{} {}" with administrator privileges with prompt "homeTier 需要管理员权限以结束后台进程""#,
            sig.trim_start_matches("SIG"),
            pid
        ))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn is_process_alive(pid: u32) -> bool {
    unsafe {
        let ret = libc::kill(pid as i32, 0);
        if ret == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn is_process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::ProcessStatus::GetExitCodeProcess;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION};
    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return false,
        };
        if handle.is_invalid() {
            return false;
        }
        let mut exit_code = 0u32;
        let ret = GetExitCodeProcess(handle, &mut exit_code);
        let _ = CloseHandle(handle);
        ret.is_ok() && exit_code == STILL_ACTIVE
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) fn is_process_alive(_pid: u32) -> bool {
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

/// 读取进程完整命令行（含参数），用于校验进程身份
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn process_command(pid: u32) -> Option<String> {
    let out = std::process::Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("command=")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let cmd = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if cmd.is_empty() { None } else { Some(cmd) }
}

/// 校验指定 PID 是否为 homeTier GUI 进程（命令含 homeTier 且不含 --daemon，
/// 用于区分同名 daemon 进程；防止 PID 复用导致 daemon 看门狗误关）
pub(crate) fn is_hometier_gui_process(pid: u32) -> bool {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        match process_command(pid) {
            Some(cmd) => cmd.to_lowercase().contains("hometier") && !cmd.contains("--daemon"),
            None => false,
        }
    }
    #[cfg(target_os = "windows")]
    {
        // Windows 看门狗退化为仅检查 PID 存活（is_process_alive 已实现）。
        // Windows 无 /proc 接口，无法可靠区分 GUI 与 daemon 进程，
        // 因此该函数在 Windows 上恒为 true，避免与 is_process_alive
        // 共同触发 OR 误杀。
        let _ = pid;
        true
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = pid;
        false
    }
}

/// 校验指定 PID 是否为 homeTier daemon 进程（命令含 homeTier 且含 --daemon，
/// 避免误杀同名 GUI 进程）
fn is_hometier_daemon_process(pid: u32) -> bool {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        match process_command(pid) {
            Some(cmd) => cmd.to_lowercase().contains("hometier") && cmd.contains("--daemon"),
            None => false,
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        false
    }
}

/// 查找监听指定 TCP 端口的进程 PID（macOS/Linux 用 lsof，Windows 用 netstat）
fn find_listener_pid(port: u16) -> Option<u32> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let port_arg = format!("-iTCP:{}", port);
        let out = std::process::Command::new("lsof")
            .args(["-nP", port_arg.as_str(), "-sTCP:LISTEN", "-t"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find_map(|l| l.trim().parse::<u32>().ok())
    }
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let suffix = format!(":{}", port);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 && parts[1].ends_with(&suffix) && parts[3] == "LISTENING" {
                if let Ok(pid) = parts[4].parse::<u32>() {
                    return Some(pid);
                }
            }
        }
        None
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = port;
        None
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
