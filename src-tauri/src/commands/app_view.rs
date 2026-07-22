use std::sync::Mutex;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, Wry,
};

pub struct AppWebview(pub Mutex<Option<WebviewWindow<Wry>>>);

#[tauri::command]
pub async fn open_app_view(
    url: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    app: AppHandle<Wry>,
    state: tauri::State<'_, AppWebview>,
) -> Result<(), String> {
    let parsed: url::Url = url.parse().map_err(|e: url::ParseError| e.to_string())?;

    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(ref wv) = *guard {
        wv.navigate(parsed).map_err(|e| e.to_string())
    } else {
        let wv = WebviewWindowBuilder::new(&app, "app-webview", WebviewUrl::External(parsed))
            .position(x as f64, y as f64)
            .inner_size(w as f64, h as f64)
            .build()
            .map_err(|e| e.to_string())?;
        *guard = Some(wv);
        Ok(())
    }
}

#[tauri::command]
pub async fn close_app_view(
    state: tauri::State<'_, AppWebview>,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(wv) = guard.take() {
        wv.close().map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn resize_app_view(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    state: tauri::State<'_, AppWebview>,
) -> Result<(), String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(ref wv) = *guard {
        wv.set_position(tauri::Position::Logical(LogicalPosition::new(x, y)))
            .map_err(|e| format!("set_position: {}", e))?;
        wv.set_size(tauri::Size::Logical(LogicalSize::new(w, h)))
            .map_err(|e| format!("set_size: {}", e))?;
    }
    Ok(())
}