#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
    } else {
        home_tier_lib::run_with_args(elevated)
    }
}
