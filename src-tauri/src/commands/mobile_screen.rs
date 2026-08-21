//! 移动端屏幕共享命令
//!
//! 桥接前端与 MobileScreenShareManager。
//! - Android 使用 MediaProjection：权限对话框/运行时权限由原生插件（HomeTierVpnServicePlugin）触发
//! - iOS 使用 ReplayKit：由 RPSystemBroadcastPickerView 引导
//! 生命周期（init/start/stop/quality/status）统一由本模块管理。

use tauri::State;
use uuid::Uuid;
use crate::screen::mobile::mod::{MobileScreenShareManager, ScreenShareConfig, ScreenQuality};

/// 移动端屏幕共享管理器状态
pub struct MobileScreenState {
    pub managers: dashmap::DashMap<String, MobileScreenShareManager>,
}

impl MobileScreenState {
    pub fn new() -> Self {
        Self {
            managers: dashmap::DashMap::new(),
        }
    }
}

fn parse_quality(s: &str) -> Result<ScreenQuality, String> {
    match s {
        "low" => Ok(ScreenQuality::Low),
        "medium" => Ok(ScreenQuality::Medium),
        "high" => Ok(ScreenQuality::High),
        "ultra" => Ok(ScreenQuality::Ultra),
        other => Err(format!("未知画质: {}", other)),
    }
}

/// 初始化移动端屏幕共享管理器
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_screen_init(
    space_id: String,
    state: State<'_, MobileScreenState>,
) -> Result<(), String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let mut manager = MobileScreenShareManager::new(ScreenShareConfig::default());
    manager.initialize().await?;

    state.managers.insert(space_id.to_string(), manager);
    crate::log_info!(format!("移动端屏幕共享初始化: space_id={}", space_id));
    Ok(())
}

/// 请求屏幕共享权限（Android MediaProjection / iOS ReplayKit 引导）
///
/// Android 上权限对话框由原生插件 `plugin:hometiervpnservice|requestScreenCapture`
/// 触发，本命令仅记录状态；iOS 上无预授权流程（由 ReplayKit 拾取器在开始共享时引导）。
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_screen_request_permission(
    space_id: String,
    state: State<'_, MobileScreenState>,
) -> Result<(), String> {
    let manager = state
        .managers
        .get_mut(&space_id)
        .ok_or_else(|| format!("屏幕共享管理器不存在: {}", space_id))?;
    manager.request_permission().await
}

/// 打开系统设置（iOS ReplayKit 需手动开启屏幕录制）
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_screen_open_settings(
    space_id: String,
    state: State<'_, MobileScreenState>,
) -> Result<(), String> {
    let manager = state
        .managers
        .get_mut(&space_id)
        .ok_or_else(|| format!("屏幕共享管理器不存在: {}", space_id))?;
    manager.open_settings().await
}

/// 开始屏幕共享
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_screen_start(
    space_id: String,
    state: State<'_, MobileScreenState>,
) -> Result<(), String> {
    let manager = state
        .managers
        .get_mut(&space_id)
        .ok_or_else(|| format!("屏幕共享管理器不存在: {}", space_id))?;
    manager.start_sharing().await
}

/// 停止屏幕共享
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_screen_stop(
    space_id: String,
    state: State<'_, MobileScreenState>,
) -> Result<(), String> {
    let manager = state
        .managers
        .get_mut(&space_id)
        .ok_or_else(|| format!("屏幕共享管理器不存在: {}", space_id))?;
    manager.stop_sharing().await
}

/// 设置屏幕共享画质（low / medium / high / ultra）
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_screen_set_quality(
    space_id: String,
    quality: String,
    state: State<'_, MobileScreenState>,
) -> Result<(), String> {
    let quality = parse_quality(&quality)?;
    let manager = state
        .managers
        .get_mut(&space_id)
        .ok_or_else(|| format!("屏幕共享管理器不存在: {}", space_id))?;
    manager.set_quality(quality).await
}

/// 获取屏幕共享状态
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_screen_get_status(
    space_id: String,
    state: State<'_, MobileScreenState>,
) -> Result<String, String> {
    let manager = state
        .managers
        .get(&space_id)
        .ok_or_else(|| format!("屏幕共享管理器不存在: {}", space_id))?;
    Ok(manager.status().as_str().to_string())
}

/// 请求相机权限（视频通话使用；Android 运行时权限由原生插件触发）
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_screen_request_camera_permission(
    space_id: String,
    state: State<'_, MobileScreenState>,
) -> Result<(), String> {
    let manager = state
        .managers
        .get_mut(&space_id)
        .ok_or_else(|| format!("屏幕共享管理器不存在: {}", space_id))?;
    manager.request_camera_permission().await
}
