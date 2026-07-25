use tauri::State;
use crate::voice::engine::VoiceManager;

#[tauri::command]
pub async fn join_voice_channel(
    space_id: String,
    voice_manager: State<'_, VoiceManager>,
) -> Result<(), String> {
    crate::log_info!(format!("加入语音频道: space_id={}", space_id));
    let engine = voice_manager.get_or_create(&space_id);
    engine.join().await
}

#[tauri::command]
pub async fn leave_voice_channel(
    space_id: String,
    voice_manager: State<'_, VoiceManager>,
) -> Result<(), String> {
    crate::log_info!(format!("离开语音频道: space_id={}", space_id));
    if let Some(engine) = voice_manager.get(&space_id) {
        engine.leave().await?;
    }
    voice_manager.remove(&space_id);
    Ok(())
}

#[tauri::command]
pub async fn toggle_mic(
    space_id: String,
    voice_manager: State<'_, VoiceManager>,
) -> Result<bool, String> {
    let engine = voice_manager.get_or_create(&space_id);
    let muted = engine.toggle_mic().await;
    crate::log_info!(format!("切换麦克风: space_id={}, muted={}", space_id, muted));
    Ok(muted)
}

#[tauri::command]
pub async fn toggle_speaker(
    space_id: String,
    voice_manager: State<'_, VoiceManager>,
) -> Result<bool, String> {
    let engine = voice_manager.get_or_create(&space_id);
    let muted = engine.toggle_speaker().await;
    crate::log_info!(format!("切换扬声器: space_id={}, muted={}", space_id, muted));
    Ok(muted)
}