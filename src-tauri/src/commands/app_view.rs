use std::sync::Mutex;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, Webview};
use tauri::webview::{WebviewBuilder, WebviewUrl};

pub struct AppWebview<R: Runtime>(pub Mutex<Option<Webview<R>>>);

#[tauri::command]
pub async fn open_app_view<R: Runtime>(
    url: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    app: AppHandle<R>,
    state: tauri::State<'_, AppWebview<R>>,
) -> Result<(), String> {
    let window = app.get_window("main").ok_or("main window not found")?;
    let parsed: url::Url = url.parse().map_err(|e: url::ParseError| e.to_string())?;

    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(ref wv) = *guard {
        wv.navigate(parsed).map_err(|e| e.to_string())
    } else {
        let wv = window
            .add_child(
                WebviewBuilder::new("app-webview", WebviewUrl::External(parsed)),
                LogicalPosition::new(x, y),
                LogicalSize::new(w, h),
            )
            .map_err(|e| e.to_string())?;
        *guard = Some(wv);
        Ok(())
    }
}

#[tauri::command]
pub async fn close_app_view<R: Runtime>(
    state: tauri::State<'_, AppWebview<R>>,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(wv) = guard.take() {
        wv.close().map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn resize_app_view<R: Runtime>(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    state: tauri::State<'_, AppWebview<R>>,
) -> Result<(), String> {
    use tauri::LogicalSize as Ls;
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(ref wv) = *guard {
        wv.reposition(tauri::Position::Logical(LogicalPosition::new(x, y)))
            .map_err(|e| format!("reposition: {}", e))?;
        wv.resize(tauri::Size::Logical(Ls::new(w, h)))
            .map_err(|e| format!("resize: {}", e))?;
    }
    Ok(())
}
