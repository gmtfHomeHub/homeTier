use crate::hotkey::platform::HotkeyManager;

#[tauri::command]
pub async fn register_hotkey(
    key: String,
    action: String,
    hotkey_manager: tauri::State<'_, HotkeyManager>,
) -> Result<(), String> {
    crate::log_info!(format!("注册快捷键: key={}, action={}", key, action));
    hotkey_manager.register(&key, &action).await
}

#[tauri::command]
pub async fn unregister_hotkey(
    key: String,
    hotkey_manager: tauri::State<'_, HotkeyManager>,
) -> Result<(), String> {
    crate::log_info!(format!("注销快捷键: key={}", key));
    hotkey_manager.unregister(&key).await
}

#[tauri::command]
pub async fn list_hotkeys(
    hotkey_manager: tauri::State<'_, HotkeyManager>,
) -> Result<Vec<(String, String)>, String> {
    Ok(hotkey_manager.list().await)
}