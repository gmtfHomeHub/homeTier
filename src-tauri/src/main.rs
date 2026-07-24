#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let elevated = args.iter().any(|a| a == "--elevated");
    let daemon = args.iter().any(|a| a == "--daemon");

    if daemon {
        home_tier_lib::run_daemon()
    } else {
        home_tier_lib::run_with_args(elevated)
    }
}
