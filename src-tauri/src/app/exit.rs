//! 应用退出清理逻辑。原 lib.rs 中 `app.run` 内 RunEvent::Exit 的清理迁移至此。

use std::sync::Arc;

use tauri::Manager;

use crate::log_info;
use crate::app::daemon;
use crate::cleanup;

/// 应用退出时的清理流程（原 lib.rs RunEvent::Exit 分支）。
pub fn on_exit_cleanup(app_handle: &tauri::AppHandle) {
    use std::thread;
    use std::time::{Duration, Instant};

    log_info!("[GUI] 应用退出，开始清理...");

    // 1. 断开所有运行中的空间
    if let Some(space_mgr) = app_handle.try_state::<Arc<crate::space::manager::SpaceManager>>() {
        let space_mgr_clone = space_mgr.inner().clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            rt.block_on(async {
                space_mgr_clone.shutdown_all().await;
            });
        })
        .join()
        .ok();
    }

    // 2. 优雅关闭 daemon
    {
        let client = crate::daemon::client::IpcClient::get_global();
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build();
        if let Ok(rt) = rt {
            rt.block_on(async {
                log_info!("[退出] 发送 IPC Shutdown 到 daemon");
                let _ = client.shutdown().await;
            });
        }
    }

    // 2.5 等待 daemon 退出（最多 8s），给 daemon 留出停止 easytier-core 的时间
    {
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            let done = daemon::get_daemon_child().map(|arc| {
                arc.lock().ok().map_or(true, |mut g| {
                    g.as_mut().map(|c| !c.is_alive()).unwrap_or(true)
                })
            }).unwrap_or(true);
            if done {
                log_info!("[GUI] daemon 已正常退出");
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }
    }

    // 3. 强制终止 daemon 进程（兜底，若仍未退出）
    if let Some(guard) = daemon::get_daemon_child() {
        if let Ok(mut handle_opt) = guard.lock() {
            if let Some(ref mut child) = handle_opt.as_mut() {
                if child.is_alive() {
                    match child.force_kill() {
                        daemon::KillOutcome::Done => {
                            log_info!("[GUI] daemon 子进程已强制终止");
                        }
                        #[cfg(target_os = "macos")]
                        daemon::KillOutcome::NeedsOsascript(pid) => {
                            crate::log_warn!(
                                format!("[GUI] daemon 未在超时内退出, 尝试 osascript 提权终止 pid={}", pid)
                            );
                            let _ = std::process::Command::new("osascript")
                                .arg("-e")
                                .arg(format!(r#"do shell script "kill -9 {}" with administrator privileges with prompt "homeTier 需要管理员权限以结束后台进程""#, pid))
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .spawn();
                        }
                    }
                    // 执行 take 移除句柄
                }
                // take
                handle_opt.take();
            }
        }
    }

    // 4. 最终清理（基于 PID 文件/端口关闭残留 daemon 与 easytier-core）
    let app_data = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let easytier_config_dir = app_data.join("easytier");
    cleanup::cleanup_all(&app_data, &easytier_config_dir);
    log_info!("[GUI] 应用退出清理完成");
}