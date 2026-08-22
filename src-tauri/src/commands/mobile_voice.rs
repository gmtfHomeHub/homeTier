//! 移动端语音命令
//!
//! 提供移动端专用的语音控制命令，桥接前端与 MobileVoiceManager

use tauri::State;
use crate::voice::mobile::MobileVoiceManager;
use crate::voice::mobile::VoiceConfig;
use uuid::Uuid;
use std::sync::Arc;

/// 移动端语音管理器状态
pub struct MobileVoiceState {
    pub managers: dashmap::DashMap<String, MobileVoiceManager>,
}

impl MobileVoiceState {
    pub fn new() -> Self {
        Self {
            managers: dashmap::DashMap::new(),
        }
    }
}

/// 初始化移动端语音管理器
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_init(
    space_id: String,
    state: State<'_, MobileVoiceState>,
) -> Result<(), String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let config = VoiceConfig {
        space_id: space_id.to_string(),
        ..Default::default()
    };

    let mut manager = MobileVoiceManager::new(space_id.to_string());
    manager.initialize().await?;

    state.managers.insert(space_id.to_string(), manager);
    crate::log_info!(format!("移动端语音初始化: space_id={}", space_id));
    Ok(())
}

/// 加入语音频道
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_join(
    space_id: String,
    state: State<'_, MobileVoiceState>,
) -> Result<(), String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let mut manager = state.managers.get_mut(&space_id.to_string())
        .ok_or_else(|| format!("语音管理器不存在: {}", space_id))?;

    manager.join().await?;
    crate::log_info!(format!("移动端加入语音: space_id={}", space_id));
    Ok(())
}

/// 离开语音频道
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_leave(
    space_id: String,
    state: State<'_, MobileVoiceState>,
) -> Result<(), String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    if let Some((_, mut manager)) = state.managers.remove(&space_id.to_string()) {
        manager.leave().await?;
    }
    crate::log_info!(format!("移动端离开语音: space_id={}", space_id));
    Ok(())
}

/// 切换麦克风静音
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_toggle_mic(
    space_id: String,
    state: State<'_, MobileVoiceState>,
) -> Result<bool, String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let mut manager = state.managers.get_mut(&space_id.to_string())
        .ok_or_else(|| format!("语音管理器不存在: {}", space_id))?;

    let muted = manager.toggle_mic().await?;
    Ok(muted)
}

/// 切换扬声器静音
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_toggle_speaker(
    space_id: String,
    state: State<'_, MobileVoiceState>,
) -> Result<bool, String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let mut manager = state.managers.get_mut(&space_id.to_string())
        .ok_or_else(|| format!("语音管理器不存在: {}", space_id))?;

    let muted = manager.toggle_speaker().await?;
    Ok(muted)
}

/// 获取麦克风静音状态
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_get_mic_status(
    space_id: String,
    state: State<'_, MobileVoiceState>,
) -> Result<bool, String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let manager = state.managers.get(&space_id.to_string())
        .ok_or_else(|| format!("语音管理器不存在: {}", space_id))?;

    Ok(manager.is_mic_muted().await)
}

/// 获取扬声器静音状态
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_get_speaker_status(
    space_id: String,
    state: State<'_, MobileVoiceState>,
) -> Result<bool, String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let manager = state.managers.get(&space_id.to_string())
        .ok_or_else(|| format!("语音管理器不存在: {}", space_id))?;

    Ok(manager.is_speaker_muted().await)
}

/// 发送音频数据 (供前端/音频引擎调用)
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_send_audio(
    space_id: String,
    data: Vec<u8>,
    state: State<'_, MobileVoiceState>,
) -> Result<(), String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let mut manager = state.managers.get_mut(&space_id.to_string())
        .ok_or_else(|| format!("语音管理器不存在: {}", space_id))?;

    manager.send_audio(&data).await
}

/// 接收音频数据 (供网络层调用)
#[tauri::command]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn mobile_voice_receive_audio(
    space_id: String,
    data: Vec<u8>,
    state: State<'_, MobileVoiceState>,
) -> Result<(), String> {
    let space_id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("无效的 space_id: {}", e))?;

    let mut manager = state.managers.get_mut(&space_id.to_string())
        .ok_or_else(|| format!("语音管理器不存在: {}", space_id))?;

    manager.receive_audio(&data).await
}