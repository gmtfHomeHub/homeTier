use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::AppHandle;

#[derive(Deserialize)]
pub struct TraySpace {
    pub id: String,
    pub name: String,
}

#[tauri::command]
pub fn update_tray_menu(app: AppHandle, spaces: Vec<TraySpace>) -> Result<(), String> {
    let menu = Menu::new(&app).map_err(|e| e.to_string())?;

    let show = MenuItem::with_id(&app, "show", "显示/隐藏", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    menu.append(&show).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator(&app)).map_err(|e| e.to_string())?;

    for space in &spaces {
        let item = MenuItem::with_id(&app, format!("space-{}", space.id), &space.name, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        menu.append(&item).map_err(|e| e.to_string())?;
    }

    if !spaces.is_empty() {
        menu.append(&PredefinedMenuItem::separator(&app)).map_err(|e| e.to_string())?;
    }
    let quit = MenuItem::with_id(&app, "quit", "退出", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    menu.append(&quit).map_err(|e| e.to_string())?;

    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }

    Ok(())
}
