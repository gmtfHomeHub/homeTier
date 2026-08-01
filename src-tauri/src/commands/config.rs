use std::collections::HashMap;

/// 获取应用配置（配置文件当前内容）
#[tauri::command]
pub fn get_app_config() -> Result<HashMap<String, String>, String> {
    match crate::config::global() {
        Some(cfg) => Ok(cfg.all()),
        None => Err("配置尚未初始化".to_string()),
    }
}

/// 更新应用配置（写入内存并落盘，立即生效；端口类配置下次 daemon 启动生效）
#[tauri::command]
pub fn set_app_config(updates: HashMap<String, String>) -> Result<(), String> {
    let cfg = crate::config::global().ok_or("配置尚未初始化")?;
    for (key, value) in &updates {
        cfg.set(key, value)?;
    }
    // LOG_ENABLED 立即生效（无需重启）
    if let Some(v) = updates.get(crate::config::KEY_LOG_ENABLED) {
        crate::log::set_log_enabled(v != "0");
    }
    Ok(())
}

/// 获取配置文件路径
#[tauri::command]
pub fn get_config_file_path() -> Result<String, String> {
    match crate::config::global() {
        Some(cfg) => Ok(cfg.path().to_string_lossy().to_string()),
        None => Err("配置尚未初始化".to_string()),
    }
}

/// 获取配置模板来源路径（打包资源目录或仓库根）
#[tauri::command]
pub fn get_config_template_path() -> Result<String, String> {
    match crate::config::global() {
        Some(cfg) => Ok(cfg
            .template_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()),
        None => Err("配置尚未初始化".to_string()),
    }
}
