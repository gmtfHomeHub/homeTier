use tauri::State;
use crate::chat::message::ChatMessage;
use crate::space::manager::SpaceManager;
use std::sync::Arc;

/// 发送 WebRTC 信令消息（offer/answer/ice 等）
///
/// `target` 为 Some 时定向发送到指定成员（目标虚拟 IP），否则广播到所有 peers。
/// 信令消息不落库，仅走聊天广播通道。
#[tauri::command]
pub async fn send_signal(
    space_id: String,
    payload: String,
    target: Option<String>,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let space_uuid = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;

    let spaces = space_manager.list().await?;
    let space = spaces.iter()
        .find(|s| s.id == space_uuid)
        .ok_or_else(|| "Space not found".to_string())?;

    let sender_id = space_uuid;
    let sender_name = gethostname::gethostname().to_string_lossy().to_string();

    let mut msg = ChatMessage::signal(space_uuid, sender_id, sender_name, payload);
    msg.sign(&space.network_secret);

    if let Some(t) = &target {
        space_manager.send_signal_to(&space_uuid, t, &msg).await?;
    } else {
        let errors = space_manager.broadcast_message(&msg).await;
        if !errors.is_empty() {
            crate::log_warn!(format!("广播信令失败: {:?}", errors));
        }
    }

    crate::log_debug!(format!("发送信令: space_id={}, target={:?}", space_id, target));
    Ok(())
}
