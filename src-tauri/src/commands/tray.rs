#![cfg(not(any(target_os = "android", target_os = "ios")))]

use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::AppHandle;

#[derive(Deserialize)]
pub struct TraySpace {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct TrayLabels {
    pub show: String,
    pub quit: String,
}

#[tauri::command]
pub fn update_tray_menu(app: AppHandle, spaces: Vec<TraySpace>, labels: TrayLabels) -> Result<(), String> {
    let menu = Menu::new(&app).map_err(|e| e.to_string())?;

    let show = MenuItem::with_id(&app, "show", &labels.show, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    menu.append(&show).map_err(|e| e.to_string())?;
    let sep1 = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    menu.append(&sep1).map_err(|e| e.to_string())?;

    for space in &spaces {
        let item = MenuItem::with_id(&app, format!("space-{}", space.id), &space.name, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        menu.append(&item).map_err(|e| e.to_string())?;
    }

    if !spaces.is_empty() {
        let sep2 = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
        menu.append(&sep2).map_err(|e| e.to_string())?;
    }
    let quit = MenuItem::with_id(&app, "quit", &labels.quit, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    menu.append(&quit).map_err(|e| e.to_string())?;

    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }

    Ok(())
}
