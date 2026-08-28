//! daemon 子进程句柄管理与启动。
//! 原 lib.rs 中的 DaemonHandle / DAEMON_CHILD / spawn_daemon 迁移至此。

use std::sync::{Arc, Mutex, OnceLock};

/// 全局 daemon 子进程引用（供 Exit 事件兜底 kill 使用，try_state 在 Exit 时可能失效）
/// macOS debug（GUI 非 root）：daemon 经 osascript 提权启动，无 Child 句柄，用 Pid 跟踪；
/// 其余场景 daemon 是 GUI 直接子进程，用 Child 跟踪。
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct DaemonChild {
    handle: DaemonHandleKind,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) enum DaemonHandleKind {
    Child(std::process::Child),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Pid(u32),
}

/// daemon 就绪标志（从后台线程标记，前端通过 Tauri command 轮询）
pub struct DaemonReadyState {
    pub ready: Arc<std::sync::atomic::AtomicBool>,
    pub reason: Arc<Mutex<Option<String>>>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
static DAEMON_CHILD: OnceLock<Arc<Mutex<Option<DaemonChild>>>> = OnceLock::new();

/// 将启动成功的 daemon 句柄存入全局（供 Exit 兜底）
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn set_daemon_child(handle: DaemonChild) {
    let inner = Arc::new(Mutex::new(Some(handle)));
    let _ = DAEMON_CHILD.set(inner);
}

/// 取全局 daemon 句柄（互斥锁引用）
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_daemon_child() -> Option<Arc<Mutex<Option<DaemonChild>>>> {
    DAEMON_CHILD.get().cloned()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl DaemonChild {
    /// 新建 Child 型句柄
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn from_child(child: std::process::Child) -> Self {
        Self { handle: DaemonHandleKind::Child(child) }
    }

    /// 新建 pid 型句柄（macOS osascript 启动 / Windows UAC 提权启动）
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub fn from_pid(pid: u32) -> Self {
        Self { handle: DaemonHandleKind::Pid(pid) }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn id(&self) -> Option<u32> {
        match &self.handle {
            DaemonHandleKind::Child(child) => Some(child.id()),
            #[cfg(target_os = "macos")]
            DaemonHandleKind::Pid(pid) => Some(*pid),
            #[cfg(target_os = "windows")]
            DaemonHandleKind::Pid(pid) => Some(*pid),
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn is_alive(&mut self) -> bool {
        match &mut self.handle {
            DaemonHandleKind::Child(child) => child.try_wait().map(|s| s.is_none()).unwrap_or(false),
            #[cfg(target_os = "macos")]
            DaemonHandleKind::Pid(pid) => unsafe {
                let ret = libc::kill(*pid as i32, 0);
                if ret == 0 {
                    return true;
                }
                std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
            },
            #[cfg(target_os = "windows")]
            DaemonHandleKind::Pid(pid) => {
                use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
                use windows::Win32::System::Threading::{
                    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
                };
                unsafe {
                    let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, *pid)
                    else {
                        return false;
                    };
                    let mut code: u32 = 0;
                    let alive = GetExitCodeProcess(handle, &mut code as *mut u32).is_ok()
                        && code == STILL_ACTIVE.0 as u32;
                    let _ = CloseHandle(handle);
                    alive
                }
            },
        }
    }

    /// 强制终止。返回是否已处理：
    /// - Child：直接 kill/wait
    /// - Pid(macOS)：返回该 pid 供上层 osascript 提权兜底
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(crate) fn force_kill(&mut self) -> KillOutcome {
        match &mut self.handle {
            DaemonHandleKind::Child(child) => {
                let _ = child.kill();
                let _ = child.wait();
                KillOutcome::Done
            }
            #[cfg(target_os = "macos")]
            DaemonHandleKind::Pid(pid) => KillOutcome::NeedsOsascript(*pid),
            #[cfg(target_os = "windows")]
            DaemonHandleKind::Pid(pid) => {
                // 提权后 daemon 为管理员进程，GUI 权限不足时 TerminateProcess 会失败；
                // 失败不报错，依赖 daemon 的 gui_pid 看门狗 + 优雅退出兜底。
                use windows::Win32::Foundation::CloseHandle;
                use windows::Win32::System::Threading::{
                    OpenProcess, TerminateProcess, PROCESS_TERMINATE,
                };
                unsafe {
                    if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, *pid) {
                        let _ = TerminateProcess(handle, 1);
                        let _ = CloseHandle(handle);
                    }
                }
                KillOutcome::Done
            },
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) enum KillOutcome {
    Done,
    #[cfg(target_os = "macos")]
    NeedsOsascript(u32),
}

/// Desktop: 启动 daemon 子进程
/// macOS 且当前进程非 root（debug/dev 模式）：经 osascript 以管理员权限启动 daemon，
/// 使 daemon 获得 root 权限，从而可以终止同样以 root 运行的 easytier-core；
/// Windows 且当前进程非管理员：经 UAC 提权启动 daemon，使 daemon 获得管理员权限，
/// 从而 easytier-core 能创建 wintun 虚拟网卡；
/// 其余场景：直接作为子进程启动。
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn spawn_daemon(
    data_dir: &std::path::Path,
    resource_dir: Option<&std::path::Path>,
) -> Result<DaemonChild, String> {
    use std::io::BufRead;
    use std::process::{Command, Stdio};

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("获取当前可执行文件路径失败: {}", e))?;

    crate::log_info!("[GUI] 启动 daemon 子进程");

    #[cfg(target_os = "macos")]
    {
        let is_root = unsafe { libc::geteuid() == 0 };
        if !is_root {
            // debug/dev 模式：GUI 未提权，经 osascript 以 root 启动 daemon
            crate::log_info!("[GUI] macOS 非 root 环境，经 osascript 提权启动 daemon");
            let log_file = data_dir.join("daemon.log");
            let script_path = std::path::PathBuf::from("/tmp/homeTier-daemon-launch.sh");
            let gui_pid_str = std::process::id().to_string();
            let resource_dir_arg = resource_dir
                .map(|d| format!(" --daemon-resource-dir \"{}\"", d.display()))
                .unwrap_or_default();
            let script_content = format!(
                r#"#!/bin/sh
"{}" --daemon --daemon-config "{}" --daemon-data "{}" --gui-pid "{}"{} < /dev/null > "{}" 2>&1 &
DAEMON_PID=$!
echo "homeTier daemon pid=$DAEMON_PID"
echo "$DAEMON_PID" > "{}/daemon.pid"
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
    [ -f "{}" ] && exit 0
    kill -0 $DAEMON_PID > /dev/null 2>&1 || exit 1
    sleep 1
done
echo "ERROR: homeTier daemon not ready after 30s" >&2
echo "=== daemon.log dump ===" >&2
cat "{}" >&2
exit 1
"#,
                current_exe.display(),
                data_dir.display(),
                data_dir.display(),
                gui_pid_str,
                resource_dir_arg,
                log_file.display(),
                data_dir.display(),
                data_dir.join("daemon_ready.signal").display(),
                log_file.display()
            );

            std::fs::write(&script_path, &script_content)
                .map_err(|e| format!("写入 daemon 启动脚本失败: {}", e))?;

            let escaped_script = script_path
                .as_path()
                .to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            let osascript_program = format!(
                "do shell script \"/bin/sh {}\" with administrator privileges with prompt \"homeTier 需要管理员权限以启动网络服务\"",
                escaped_script
            );

            crate::log_info!("[GUI] 正在弹出授权对话框以启动 daemon...");

            let output = Command::new("osascript")
                .arg("-e")
                .arg(&osascript_program)
                .output()
                .map_err(|e| {
                    let msg = format!("macOS 提权启动 daemon 失败: {}", e);
                    crate::log_error!(&msg);
                    msg
                })?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            crate::log_info!(format!("[GUI] osascript stdout: {}", stdout.trim()));
            if !stderr_str.is_empty() {
                if stderr_str.contains("User canceled") || stderr_str.contains("canceled") {
                    return Err("用户取消了授权".to_string());
                }
                crate::log_error!(format!("[GUI] osascript stderr: {}", stderr_str));
                crate::log_error!(format!("[GUI] daemon.log 内容: {}",
                    std::fs::read_to_string(&log_file).unwrap_or_else(|_| "(无法读取)".into())
                ));
                return Err(format!("daemon 启动脚本失败: {}", stderr_str));
            }

            if !output.status.success() {
                let log_content = std::fs::read_to_string(&log_file)
                    .unwrap_or_else(|_| "(无法读取)".into());
                crate::log_error!(format!("[GUI] daemon.log: {}", log_content));
                return Err(format!("daemon 启动脚本退出码: {}", output.status));
            }

            let daemon_pid = stdout
                .lines()
                .find_map(|l| l.trim().strip_prefix("homeTier daemon pid="))
                .and_then(|s| s.trim().parse::<u32>().ok());
            let pid = daemon_pid.ok_or_else(|| {
                crate::log_error!("[GUI] 未能从 osascript 输出解析 daemon PID");
                "解析 daemon PID 失败".to_string()
            })?;
            crate::log_info!(format!("[GUI] daemon 提权启动成功, pid={}", pid));
            return Ok(DaemonChild::from_pid(pid));
        }
    }

    let mut cmd = Command::new(&current_exe);
    cmd.arg("--daemon")
        .arg("--daemon-config")
        .arg(data_dir)
        .arg("--daemon-data")
        .arg(data_dir)
        .arg("--gui-pid")
        .arg(std::process::id().to_string());
    if let Some(rd) = resource_dir {
        cmd.arg("--daemon-resource-dir").arg(rd);
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        // Windows: 非管理员时经 UAC 提权启动 daemon，使 easytier-core 能创建 wintun 虚拟网卡。
        // 已管理员则走普通子进程路径（保留 stderr 转发 + Child 句柄）。
        if !is_elevated() {
            crate::log_info!("[GUI] Windows 非管理员，经 UAC 提权启动 daemon...");
            return spawn_daemon_elevated(&current_exe, data_dir, resource_dir);
        }

        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);

        // Windows: 启动前验证关键 DLL 是否存在且有效，缺失则直接报错
        if let Some(rd) = resource_dir {
            let rd_buf = rd.to_path_buf();
            for dll_name in &["packet.dll", "wpcap.dll", "wintun.dll"] {
                let mut dll_found = false;
                for candidate_dir in &[
                    rd_buf.join("resources").join("bin"),
                    rd_buf.join("bin"),
                    rd_buf.clone(),
                ] {
                    let src = candidate_dir.join(dll_name);
                    if src.exists() {
                        // 忽略占位文件（<10KB 视为无效）
                        if let Ok(meta) = std::fs::metadata(&src) {
                            if meta.len() >= 10000 {
                                dll_found = true;
                                break;
                            }
                        }
                    }
                }
                if !dll_found {
                    return Err(format!("Windows 关键依赖 {} 缺失或无效，请确保 resource_dir 包含有效的 {} (>=10KB)", dll_name, dll_name));
                }
            }
        }
    }

    let mut child = cmd.spawn().map_err(|e| {
        let msg = format!("启动 daemon 失败: {}", e);
        crate::log_error!(&msg);
        msg
    })?;

    // 将 daemon 子进程的 stderr 转发到 GUI 日志
    if let Some(stderr) = child.stderr.take() {
        let reader = std::io::BufReader::new(stderr);
        std::thread::spawn(move || {
            for line in reader.lines() {
                if let Ok(l) = line {
                    crate::log_info!(format!("[Daemon-stderr] {}", l));
                }
            }
        });
    }

    Ok(DaemonChild::from_child(child))
}

/// Windows: 经 UAC 提权启动 daemon 子进程。
/// 用 ShellExecuteExW + verb="runas" 弹出 UAC，daemon 以管理员权限运行；
/// 提权后 daemon 非 GUI 子进程，用 Pid 跟踪（依赖 gui_pid 看门狗 + IPC 优雅退出）。
#[cfg(target_os = "windows")]
fn spawn_daemon_elevated(
    current_exe: &std::path::Path,
    data_dir: &std::path::Path,
    resource_dir: Option<&std::path::Path>,
) -> Result<DaemonChild, String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_CANCELLED};
    use windows::Win32::System::Threading::GetProcessId;
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SHELLEXECUTEINFOW, SEE_MASK_NOCLOSEPROCESS,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    let gui_pid_str = std::process::id().to_string();
    // 拼接命令行参数（路径含空格需加引号）
    let mut params = format!(
        "--daemon --daemon-config \"{}\" --daemon-data \"{}\" --gui-pid {}",
        data_dir.display(),
        data_dir.display(),
        gui_pid_str
    );
    if let Some(rd) = resource_dir {
        params.push_str(&format!(" --daemon-resource-dir \"{}\"", rd.display()));
    }

    let exe_w = to_wide(&current_exe.to_string_lossy());
    let params_w = to_wide(&params);
    let verb_w = to_wide("runas");
    let dir_w = to_wide(&current_exe.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default());

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb_w.as_ptr()),
        lpFile: PCWSTR(exe_w.as_ptr()),
        lpParameters: PCWSTR(params_w.as_ptr()),
        lpDirectory: PCWSTR(dir_w.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };

    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok.is_err() {
        let err = unsafe { GetLastError() };
        if err == ERROR_CANCELLED {
            return Err("用户取消了 UAC 授权".to_string());
        }
        return Err(format!("UAC 提权启动 daemon 失败 (err={})", err.0));
    }

    let hprocess = info.hProcess;
    let pid = unsafe { GetProcessId(hprocess) };
    // 不立即 CloseHandle：is_alive/force_kill 会用 pid 重新 OpenProcess；此处释放句柄避免泄漏。
    let _ = unsafe { CloseHandle(hprocess) };

    if pid == 0 {
        return Err("UAC 提权启动后无法获取 daemon PID".to_string());
    }
    crate::log_info!(format!("[GUI] daemon 提权启动成功, pid={}", pid));
    Ok(DaemonChild::from_pid(pid))
}

/// Windows: 检测当前进程是否以管理员权限运行。
#[cfg(target_os = "windows")]
fn is_elevated() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret_len: u32 = 0;
        let res = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut std::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len as *mut u32,
        );
        let _ = CloseHandle(token);
        res.is_ok() && elevation.TokenIsElevated != 0
    }
}