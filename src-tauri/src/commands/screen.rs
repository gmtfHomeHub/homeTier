use tauri::State;
use crate::screen::share::ScreenShareEngine;
use std::sync::Arc;

#[tauri::command]
pub async fn start_screen_share(
    screen_share: State<'_, Arc<ScreenShareEngine>>,
) -> Result<(), String> {
    screen_share.start().await
}

#[tauri::command]
pub async fn stop_screen_share(
    screen_share: State<'_, Arc<ScreenShareEngine>>,
) -> Result<(), String> {
    screen_share.stop().await
}

#[tauri::command]
pub async fn is_screen_sharing(
    screen_share: State<'_, Arc<ScreenShareEngine>>,
) -> Result<bool, String> {
    Ok(screen_share.is_active().await)
}