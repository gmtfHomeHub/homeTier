#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// macOS 生产版不再自我提权（S3），GUI 保持普通用户权限；这些函数仅 Windows/Linux 使用。
#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn is_elevated() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Security::IsUserAnAdmin;
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
        use std::os::windows::process::CommandExt;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
        let exe = exe.to_string_lossy();
        let exe_wide: Vec<u16> = exe.encode_utf16().chain(['\0' as u16]).collect();
        let args_wide: Vec<u16> = "--elevated\0".encode_utf16().collect();
        unsafe {
            ShellExecuteW(
                None,
                &windows::core::w!("runas"),
                &exe_wide,
                Some(&args_wide),
                None,
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
    let args: Vec<String> = std::env::args().collect();
    
    // --server 模式（Web 管理界面 + REST API）
    if args.iter().any(|a| a == "--server") {
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

    let daemon = args.iter().any(|a| a == "--daemon");

    if daemon {
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