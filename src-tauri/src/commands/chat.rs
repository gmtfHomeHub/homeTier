use tauri::State;
use crate::types::Message;
use crate::chat::message::ChatMessage;
use crate::db::Database;
use crate::space::manager::SpaceManager;
use std::sync::Arc;

#[tauri::command]
pub async fn send_message(
    space_id: String,
    content: String,
    msg_type: String,
    db: State<'_, Arc<Database>>,
    space_manager: State<'_, Arc<SpaceManager>>,
) -> Result<Message, String> {
    let space_uuid = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;

    // 获取空间信息以获取真实的 network-secret
    let spaces = space_manager.list().await?;
    let space = spaces.iter()
        .find(|s| s.id == space_uuid)
        .ok_or_else(|| "Space not found".to_string())?;

    // 使用机器名作为发送者标识（实际应使用用户身份系统）
    let sender_id = space_uuid;
    let sender_name = gethostname::gethostname().to_string_lossy().to_string();

    let mut msg = match msg_type.as_str() {
        "image" => ChatMessage::image(space_uuid, sender_id, sender_name, content),
        _ => ChatMessage::text(space_uuid, sender_id, sender_name, content),
    };
    // 使用真实的 network-secret 签名
    msg.sign(&space.network_secret);

    // 保存到数据库
    let row = crate::db::models::MessageRow {
        id: msg.id.to_string(),
        space_id: msg.space_id.to_string(),
        sender_id: msg.sender_id.to_string(),
        sender_name: msg.sender_name.clone(),
        msg_type: msg.msg_type.clone(),
        content: msg.content.clone(),
        timestamp: msg.timestamp.to_rfc3339(),
        status: "sent".to_string(),
    };
    db.insert_message(&row)?;

    // 广播消息到所有 peers
    let errors = space_manager.broadcast_message(&msg).await;
    if !errors.is_empty() {
        crate::log_warn!(format!("广播消息失败: {:?}", errors));
    }

    crate::log_info!(format!("发送消息: space_id={}, type={}", space_id, msg_type));
    // 返回给前端
    Ok(Message {
        id: msg.id,
        space_id: msg.space_id,
        sender_id: msg.sender_id,
        sender_name: msg.sender_name,
        msg_type: crate::types::MessageType::Text,
        content: msg.content,
        timestamp: msg.timestamp,
        status: crate::types::MessageStatus::Sent,
    })
}

#[tauri::command]
pub async fn get_message_history(
    space_id: String,
    limit: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Message>, String> {
    let limit = limit.unwrap_or(50);
    crate::log_debug!(format!("查询消息历史: space_id={}, limit={}", space_id, limit));
    let rows = db.get_messages(&space_id, limit)?;

    let messages = rows.iter().map(|r| {
        let msg_type = match r.msg_type.as_str() {
            "image" => crate::types::MessageType::Image,
            "system" => crate::types::MessageType::System,
            _ => crate::types::MessageType::Text,
        };
        let status = match r.status.as_str() {
            "sending" => crate::types::MessageStatus::Sending,
            "delivered" => crate::types::MessageStatus::Delivered,
            "failed" => crate::types::MessageStatus::Failed,
            _ => crate::types::MessageStatus::Sent,
        };
        Message {
            id: r.id.parse().unwrap_or_default(),
            space_id: r.space_id.parse().unwrap_or_default(),
            sender_id: r.sender_id.parse().unwrap_or_default(),
            sender_name: r.sender_name.clone(),
            msg_type,
            content: r.content.clone(),
            timestamp: chrono::DateTime::parse_from_rfc3339(&r.timestamp)
                .map(|d| d.with_timezone(&chrono::Local))
                .unwrap_or_else(|_| chrono::Local::now()),
            status,
        }
    }).collect();

    Ok(messages)
}