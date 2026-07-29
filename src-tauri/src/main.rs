#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
    #[cfg(target_os = "macos")]
    {
        return std::process::Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "do shell script \"\\\"{}\" --elevated\" with administrator privileges",
                exe.display()
            ))
            .spawn()
            .is_ok();
    }
    #[cfg(target_os = "linux")]
    {
        return std::process::Command::new("/usr/bin/pkexec")
            .arg("--disable-internal-agent")
            .arg(exe.to_string_lossy())
            .arg("--elevated")
            .spawn()
            .is_ok();
    }
    #[allow(unreachable_code)]
    false
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let elevated = args.iter().any(|a| a == "--elevated");
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
        home_tier_lib::run_daemon(config_dir, data_dir)
    } else if !elevated && !is_elevated() {
        if elevate_self() {
            std::process::exit(0);
        }
        home_tier_lib::run_with_args(false)
    } else {
        home_tier_lib::run_with_args(elevated)
    }
}
