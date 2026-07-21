use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::Manager;

/// 全局快捷键管理器
pub struct HotkeyManager {
    shortcuts: Arc<RwLock<HashMap<String, String>>>, // key -> action
    app_handle: Arc<RwLock<Option<tauri::AppHandle>>>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        let mut shortcuts = HashMap::new();
        shortcuts.insert("Ctrl+M".to_string(), "toggle_mic".to_string());
        shortcuts.insert("Ctrl+T".to_string(), "toggle_speaker".to_string());
        Self {
            shortcuts: Arc::new(RwLock::new(shortcuts)),
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// 初始化（传入 AppHandle）
    pub fn init(&self, app: &tauri::AppHandle) {
        *self.app_handle.blocking_write() = Some(app.clone());
    }

    /// 注册快捷键
    pub async fn register(&self, key: &str, action: &str) -> Result<(), String> {
        self.shortcuts.write().await.insert(key.to_string(), action.to_string());

        // 通过 Tauri 全局快捷键 API 注册
        if let Some(handle) = self.app_handle.read().await.as_ref() {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if let Ok(shortcut) = key.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    handle.global_shortcut().register(shortcut)
                        .map_err(|e| format!("注册快捷键失败: {}", e))?;
                }
            }
        }

        Ok(())
    }

    /// 注销快捷键
    pub async fn unregister(&self, key: &str) -> Result<(), String> {
        self.shortcuts.write().await.remove(key);

        // 通过 Tauri 全局快捷键 API 注销
        if let Some(handle) = self.app_handle.read().await.as_ref() {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if let Ok(shortcut) = key.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    handle.global_shortcut().unregister(shortcut)
                        .map_err(|e| format!("注销快捷键失败: {}", e))?;
                }
            }
        }

        Ok(())
    }

    /// 获取所有快捷键
    pub async fn list(&self) -> Vec<(String, String)> {
        self.shortcuts.read().await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// 处理快捷键事件
    pub async fn handle_action(&self, action: &str) -> Option<String> {
        // 返回 action 供上层处理
        Some(action.to_string())
    }
}