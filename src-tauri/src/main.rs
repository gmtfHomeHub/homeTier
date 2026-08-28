#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

/// 安装崩溃诊断 hook：panic 时写入 crash.log 并弹窗（Windows），
/// 避免“启动即闪退且无法查看日志”。crash.log 双路径写入：
/// 优先 %APPDATA%/com.hometier.app/crash.log，其次当前 exe 目录 crash.log。
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{:?}", info.payload())
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "?".into());
        let bt = std::backtrace::Backtrace::force_capture();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let full = format!(
            "[homeTier panic] ts={}\nthread: {:?}\nmessage: {}\nlocation: {}\n\nBacktrace:\n{}",
            ts,
            std::thread::current().name(),
            payload,
            location,
            bt
        );

        let mut written = false;
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let dir = std::path::Path::new(&appdata).join("com.hometier.app");
                if std::fs::create_dir_all(&dir).is_ok()
                    && std::fs::write(dir.join("crash.log"), &full).is_ok()
                {
                    written = true;
                }
            }
        }
        if !written {
            if let Ok(exe) = std::env::current_exe() {
                if let Some(p) = exe.parent() {
                    let _ = std::fs::write(p.join("crash.log"), &full);
                }
            }
        }

        // Windows 弹窗显示崩溃信息（截断避免超长）
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
            use windows::core::PCWSTR;
            let shown: String = if full.len() > 6000 {
                format!("{}...\n\n(完整信息见 crash.log)", &full[..6000])
            } else {
                full.clone()
            };
            let title: Vec<u16> = "homeTier 启动崩溃".encode_utf16().chain(Some(0)).collect();
            let msg: Vec<u16> = shown.encode_utf16().chain(Some(0)).collect();
            unsafe {
                let _ = MessageBoxW(
                    None,
                    PCWSTR::from_raw(msg.as_ptr()),
                    PCWSTR::from_raw(title.as_ptr()),
                    MB_OK | MB_ICONERROR,
                );
            }
        }

        default_hook(info);
    }));
}

// macOS 生产版不再自我提权（S3），GUI 保持普通用户权限；这些函数仅 Windows/Linux 使用。
#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn is_elevated() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Shell::IsUserAnAdmin;
        unsafe { IsUserAnAdmin().as_bool() }
    }
    #[cfg(not(windows))]
    {
        unsafe { libc::geteuid() == 0 }
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn elevate_self() -> bool {
    let exe = std::env::current_exe().unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
        use windows::core::PCWSTR;
        let exe = exe.to_string_lossy();
        let exe_wide: Vec<u16> = exe.encode_utf16().chain(['\0' as u16]).collect();
        let args_wide: Vec<u16> = "--elevated\0".encode_utf16().collect();
        unsafe {
            ShellExecuteW(
                None,
                windows::core::w!("runas"),
                PCWSTR::from_raw(exe_wide.as_ptr()),
                PCWSTR::from_raw(args_wide.as_ptr()),
                PCWSTR::null(),
                SW_HIDE,
            );
        }
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        return std::process::Command::new("/usr/bin/pkexec")
            .arg("--disable-internal-agent")
            .arg(exe.to_string_lossy().into_owned())
            .arg("--elevated")
            .spawn()
            .is_ok();
    }
    #[allow(unreachable_code)]
    false
}

fn main() -> std::process::ExitCode {
    install_panic_hook();
    let args: Vec<String> = std::env::args().collect();
    
    // --server 模式（Web 管理界面 + REST API）
    if args.iter().any(|a| a == "--server") {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let bind = args
                .iter()
                .position(|a| a == "--server-bind")
                .and_then(|i| args.get(i + 1).cloned());
            let port = args
                .iter()
                .position(|a| a == "--server-port")
                .and_then(|i| args.get(i + 1).and_then(|s| s.parse().ok()));
            let server_dir = args
                .iter()
                .position(|a| a == "--server-dir")
                .and_then(|i| args.get(i + 1).map(std::path::PathBuf::from));
            let resource_dir = args
                .iter()
                .position(|a| a == "--server-resource-dir")
                .and_then(|i| args.get(i + 1).map(std::path::PathBuf::from));
            let static_dir = args
                .iter()
                .position(|a| a == "--server-static-dir")
                .and_then(|i| args.get(i + 1).map(std::path::PathBuf::from));
            
            return home_tier_lib::run_server(bind, port, server_dir, resource_dir, static_dir);
        }
    }

    let daemon = args.iter().any(|a| a == "--daemon");

    if daemon {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let config_dir = args
                .iter()
                .position(|a| a == "--daemon-config")
                .and_then(|i| args.get(i + 1))
                .map(|s| std::path::PathBuf::from(s))
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().join("homeTier"));
            let data_dir = args
                .iter()
                .position(|a| a == "--daemon-data")
                .and_then(|i| args.get(i + 1))
                .map(|s| std::path::PathBuf::from(s))
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let gui_pid = args
                .iter()
                .position(|a| a == "--gui-pid")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.parse::<u32>().ok());
            let resource_dir = args
                .iter()
                .position(|a| a == "--daemon-resource-dir")
                .and_then(|i| args.get(i + 1))
                .map(std::path::PathBuf::from);
            home_tier_lib::run_daemon(config_dir, data_dir, gui_pid, resource_dir)
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            std::process::ExitCode::FAILURE
        }
    } else {
        #[cfg(debug_assertions)]
        return home_tier_lib::run_with_args(false);
        #[cfg(not(debug_assertions))]
        {
            #[cfg(target_os = "macos")]
            {
                // macOS 生产版：GUI 不再自我提权（保持普通用户权限，降低 WebView 权限面），
                // 仅 daemon 经 osascript 提权运行（与 dev 路径一致）。
                // daemon 生命周期由 S5 看门狗（--gui-pid）与 S1 启动清理兜底保障。
                home_tier_lib::run_with_args(false)
            }
            #[cfg(not(target_os = "macos"))]
            {
                let elevated = args.iter().any(|a| a == "--elevated");
                if !elevated && !is_elevated() {
                    if elevate_self() {
                        std::process::exit(0);
                    }
                    home_tier_lib::run_with_args(false)
                } else {
                    home_tier_lib::run_with_args(elevated)
                }
            }
        }
    }
}