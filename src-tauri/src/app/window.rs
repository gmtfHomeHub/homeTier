//! 窗口显隐、激活辅助函数。原 lib.rs 中的 activate_main_window / toggle_window_visibility / ELEVATED 迁移至此。


use tauri::Manager;

/// UAC / macOS 提权标记，用于检测当前进程是否通过提权启动
#[cfg(any(target_os = "windows", target_os = "macos"))]
static ELEVATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn set_elevated(val: bool) {
    use std::sync::atomic::Ordering;
    ELEVATED.store(val, Ordering::SeqCst);
}

/// 检查当前进程是否以提权模式运行
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn is_elevated_process() -> bool {
    use std::sync::atomic::Ordering;
    ELEVATED.load(Ordering::SeqCst)
}

/// macOS：必须应用激活到上层并聚焦主窗口。
/// `set_focus()` 仅调用 makeKeyWindow，无法激活应用进程；从 Accessory（托盘态）
/// 切回 Regular 后必须显式调用 NSRunningApplication activateWithOptions 才能置顶。
#[cfg(target_os = "macos")]
pub fn activate_main_window(app: &tauri::AppHandle) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        unsafe {
            let current = NSRunningApplication::currentApplication();
            let _ = current
                .activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
        }
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    });
}

#[cfg(not(target_os = "android"))]
pub fn toggle_window_visibility(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(true) {
            let _ = window.hide();
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                let _ = app.set_activation_policy(ActivationPolicy::Accessory);
            }
        } else {
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                let _ = app.set_activation_policy(ActivationPolicy::Regular);
                activate_main_window(app);
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
    }
}