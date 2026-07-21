use tauri::State;
use crate::voice::engine::VoiceManager;

#[tauri::command]
pub async fn join_voice_channel(
    space_id: String,
    voice_manager: State<'_, VoiceManager>,
) -> Result<(), String> {
    let engine = voice_manager.get_or_create(&space_id);
    engine.join().await
}

#[tauri::command]
pub async fn leave_voice_channel(
    space_id: String,
    voice_manager: State<'_, VoiceManager>,
) -> Result<(), String> {
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
    Ok(engine.toggle_mic().await)
}

#[tauri::command]
pub async fn toggle_speaker(
    space_id: String,
    voice_manager: State<'_, VoiceManager>,
) -> Result<bool, String> {
    let engine = voice_manager.get_or_create(&space_id);
    Ok(engine.toggle_speaker().await)
}