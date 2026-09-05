use std::sync::Arc;
use uuid::Uuid;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, Router},
    Json,
};
use chrono;
use tower_http::services::ServeDir;

use crate::chat::message::ChatMessage;
use crate::platform::machine_id;
use crate::easytier::config::NetworkConfig;
use crate::server::ws::{ws_handler, ws_events_handler};
use crate::server::AppState;
use crate::types::FileInfo;

pub fn cmd_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/ping", get(ping_handler))
        // 空间
        .route("/space/create", post(create_space_handler))
        .route("/space/join", post(join_space_handler))
        .route("/space/list", get(list_spaces_handler))
        .route("/space/{space_id}", get(get_space_handler).delete(delete_space_handler))
        .route("/space/{space_id}/leave", post(leave_space_handler))
        .route("/space/{space_id}/connect", post(connect_space_handler))
        .route("/space/{space_id}/disconnect", post(disconnect_space_handler))
        .route("/space/{space_id}/status", get(space_status_handler))
        .route("/space/{space_id}/members", get(list_members_handler))
        .route("/space/{space_id}/config", get(get_space_config_handler).post(update_space_config_handler))
        .route("/space/{space_id}/config/patch", post(patch_space_config_handler))
        .route("/space/{space_id}/share", post(generate_share_link_handler))
        .route("/qr/parse", post(parse_qr_handler))
        .route("/space/share/parse-data", post(parse_share_data_handler))
        .route("/space/{space_id}/signal", post(send_signal_handler))
        .route("/space/{space_id}/acl", get(get_acl_rules_handler).post(create_acl_rule_handler))
        .route("/space/{space_id}/acl/update", post(update_acl_rule_handler))
        .route("/space/{space_id}/acl/delete", post(delete_acl_rule_handler))
        .route("/space/{space_id}/port-forwards", get(get_port_forward_rules_handler).post(create_port_forward_rule_handler))
        .route("/space/{space_id}/port-forwards/update", post(update_port_forward_rule_handler))
        .route("/space/{space_id}/port-forwards/delete", post(delete_port_forward_rule_handler))
        // 应用管理
        .route("/space/{space_id}/apps", get(list_apps_handler).post(add_app_handler))
        .route("/space/{space_id}/apps/share", post(share_app_handler))
        .route("/space/{space_id}/apps/update", post(update_app_handler))
        .route("/space/{space_id}/apps/delete", post(delete_app_handler))
        // 系统应用
        .route("/system/apps", get(get_system_apps_handler))
        // 聊天
        .route("/chat/{space_id}/history", get(get_message_history_handler))
        .route("/chat/{space_id}/send", post(send_message_handler))
        // 网络
        .route("/network/{space_id}/stats", get(get_network_stats_handler))
        .route("/network/{space_id}/peers", get(get_space_peers_handler))
        // 日志
        .route("/log/list", get(get_logs_handler))
        .route("/log/space/{space_id}", get(get_space_logs_handler))
        .route("/log/clear", post(clear_logs_handler))
        .route("/log/query", get(query_logs_handler))
        .route("/log/modules", get(get_log_modules_handler))
        .route("/log/clear-filtered", post(clear_logs_filtered_handler))
        .route("/log/export", get(export_logs_handler))
        // 配置
        .route("/config/system", get(get_system_config_handler).post(set_system_config_handler))
        .route("/config/app", get(get_app_config_handler).post(set_app_config_handler))
        .route("/config/path", get(get_config_path_handler))
        .route("/config/template-path", get(get_config_template_path_handler))
        // 配置存储（P2P 分布式配置同步）
        .route("/config-store/{name}/version", get(get_config_version_handler))
        .route("/config-store/{name}/download", get(download_config_handler))
        .route("/config-store/upload", post(upload_config_handler))
        .route("/config-store/remote/version", get(get_remote_config_version_handler))
        .route("/config-store/remote/download", get(download_remote_config_handler))
        // 系统信息
        .route("/system/version", get(get_app_version_handler))
        .route("/system/binary-check", get(check_easytier_binary_handler))
        .route("/system/check-update", get(check_app_update_handler))
        .route("/system/upgrade-app", post(upgrade_app_handler))
        // Proxy
        .route("/proxy/url", get(get_proxy_url_handler))
        .route("/proxy/status", get(get_proxy_status_handler))
        .route("/proxy/register", post(register_proxy_key_handler))
        .route("/proxy/source", post(set_proxy_source_handler))
        .route("/proxy/device", post(set_device_mode_handler))
        .route("/proxy/downloads", get(get_pending_downloads_handler))
        // EasyTier
        .route("/easytier/check-update", get(check_easytier_update_handler))
        .route("/easytier/upgrade", post(upgrade_easytier_handler))
        // 文件传输
        .route("/file/send", post(send_file_handler))
        .route("/file/{space_id}/download/{file_id}", get(receive_file_handler))
        .route("/file/record", post(record_received_file_handler))
        .route("/file/delete", post(delete_file_handler))
        .route("/space/{space_id}/file/list", get(list_files_handler))
        .route("/file/progress", get(get_transfer_progress_handler))
        // WebSocket
        .route("/ws/signal/{space_id}", get(ws_handler))
        .route("/ws/events", get(ws_events_handler))
        .with_state(app_state)
}

async fn parse_uuid(s: &str) -> Result<uuid::Uuid, (StatusCode, String)> {
    uuid::Uuid::parse_str(s).map_err(|e| (StatusCode::BAD_REQUEST, format!("无效 UUID: {}", e)))
}

// ---- ping ----
async fn ping_handler() -> &'static str {
    "pong"
}

// ---- 空间 ----
async fn create_space_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = body["network_name"].as_str().unwrap_or("").to_string();
    let secret = body["network_secret"].as_str().unwrap_or("").to_string();
    if name.is_empty() || secret.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 network_name 或 network_secret").into_response();
    }
    let desc = body["description"].as_str().map(|s| s.to_string());
    match state.space_manager.create(name, secret, desc).await {
        Ok(space) => {
            // 广播空间创建事件
            let event = crate::server::event::ServerEvent::new(
                crate::server::event::EventType::SpaceCreated,
                Some(space.id.to_string()),
                serde_json::json!({ "space": space }),
            );
            state.event_bus.broadcast(event).await;
            Json(space).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn join_space_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let network_name = body["network_name"].as_str().unwrap_or("").to_string();
    let secret = body["network_secret"].as_str().unwrap_or("").to_string();
    if network_name.is_empty() || secret.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 network_name 或 network_secret").into_response();
    }
    let mut config = NetworkConfig::default();
    config.network_name = network_name;
    config.network_secret = secret;
    if let Some(ip) = body["virtual_ipv4"].as_str() {
        config.virtual_ipv4 = ip.to_string();
    }
    let display_name = body["name"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    match state.space_manager.join(config, display_name).await {
        Ok(space) => Json(space).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn list_spaces_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.space_manager.list().await {
        Ok(spaces) => Json(spaces).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_space_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    let spaces = match state.space_manager.list().await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    let uuid = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    match spaces.into_iter().find(|s| s.id == uuid) {
        Some(space) => Json(space).into_response(),
        None => (StatusCode::NOT_FOUND, "空间不存在").into_response(),
    }
}

async fn delete_space_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    match state.space_manager.delete(&id).await {
        Ok(()) => {
            // 广播空间删除事件
            let event = crate::server::event::ServerEvent::new(
                crate::server::event::EventType::SpaceDeleted,
                Some(space_id.clone()),
                serde_json::json!({ "space_id": space_id }),
            );
            state.event_bus.broadcast(event).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn leave_space_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    match state.space_manager.leave(&id).await {
        Ok(()) => {
            let event = crate::server::event::ServerEvent::new(
                crate::server::event::EventType::MemberLeft,
                Some(space_id.clone()),
                serde_json::json!({ "space_id": space_id }),
            );
            state.event_bus.broadcast(event).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn connect_space_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    match state.space_manager.connect(&id).await {
        Ok(()) => {
            let event = crate::server::event::ServerEvent::new(
                crate::server::event::EventType::SpaceUpdated,
                Some(space_id.clone()),
                serde_json::json!({ "space_id": space_id, "action": "connect" }),
            );
            state.event_bus.broadcast(event).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn disconnect_space_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    match state.space_manager.disconnect(&id).await {
        Ok(()) => {
            let event = crate::server::event::ServerEvent::new(
                crate::server::event::EventType::SpaceUpdated,
                Some(space_id.clone()),
                serde_json::json!({ "space_id": space_id, "action": "disconnect" }),
            );
            state.event_bus.broadcast(event).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn space_status_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    match state.space_manager.get_space_status(&space_id).await {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "空间不存在").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn list_members_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    match state.space_manager.list_members(&id).await {
        Ok(members) => Json(members).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_space_config_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_space_config(&space_id) {
        Ok(Some(c)) => Json(c).into_response(),
        Ok(None) => Json(serde_json::Value::Null).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn update_space_config_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let config_json = body["config_json"].as_str().unwrap_or("").to_string();
    if config_json.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 config_json").into_response();
    }
    match state.db.update_space_config(&space_id, &config_json) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn patch_space_config_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(patch): Json<serde_json::Value>,
) -> impl IntoResponse {
    match state.space_manager.patch_config(&space_id, patch).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn generate_share_link_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    let ip = body["ip"].as_str().map(|s| s.to_string());
    match state.space_manager.generate_share_link(&id, ip).await {
        Ok(link) => Json(serde_json::json!({ "link": link })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn parse_qr_handler(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let link = body["link"].as_str().unwrap_or("").to_string();
    if link.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 link").into_response();
    }
    match crate::qr::decrypt_qr(&link) {
        Ok((event, data)) => {
            use base64::engine::general_purpose::STANDARD;
            use base64::Engine as _;
            Json(serde_json::json!({
                "event": event,
                "data": STANDARD.encode(&data),
            }))
            .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn parse_share_data_handler(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let data = body["data"].as_str().unwrap_or("").to_string();
    if data.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 data").into_response();
    }
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    match STANDARD.decode(&data) {
        Ok(bytes) => match crate::space::share::decode_share_binary(&bytes) {
            Ok(info) => Json(info).into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            format!("分享数据解码失败: {}", e),
        )
            .into_response(),
    }
}

// ---- 聊天 ----
async fn get_message_history_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let limit = params["limit"].as_u64().map(|v| v as u32).unwrap_or(50);
    match state.db.get_messages(&space_id, limit) {
        Ok(rows) => {
            let messages: Vec<crate::types::Message> = rows
                .into_iter()
                .map(|r| {
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
                    crate::types::Message {
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
                })
                .collect();
            Json(messages).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn send_message_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let content = body["content"].as_str().unwrap_or("").to_string();
    let msg_type = body["msg_type"].as_str().unwrap_or("text").to_string();
    if content.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 content").into_response();
    }
    let space_uuid = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    let spaces = match state.space_manager.list().await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    let space = match spaces.into_iter().find(|s| s.id == space_uuid) {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "空间不存在").into_response(),
    };
    let sender_id = space_uuid;
    let sender_name = gethostname::gethostname().to_string_lossy().to_string();
    let mut msg = match msg_type.as_str() {
        "image" => crate::chat::message::ChatMessage::image(space_uuid, sender_id, sender_name, content),
        _ => crate::chat::message::ChatMessage::text(space_uuid, sender_id, sender_name, content),
    };
    msg.sign(&space.network_secret);
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
    if let Err(e) = state.db.insert_message(&row) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
    }
    let errors = state.space_manager.broadcast_message(&msg).await;
    let peer_count = state.space_manager.chat_peer_count(&space_uuid).await;
    let status = if peer_count == 0 {
        crate::types::MessageStatus::Sent
    } else if errors.len() < peer_count {
        crate::types::MessageStatus::Delivered
    } else {
        crate::types::MessageStatus::Failed
    };
    let status_str = match status {
        crate::types::MessageStatus::Sending => "sending",
        crate::types::MessageStatus::Sent => "sent",
        crate::types::MessageStatus::Delivered => "delivered",
        crate::types::MessageStatus::Failed => "failed",
    };
    let _ = state.db.update_message_status(&msg.id.to_string(), status_str);

    // 广播到 WebSocket 客户端
    let event = crate::server::event::ServerEvent::new(
        crate::server::event::EventType::MessageSent,
        Some(space_id.clone()),
        serde_json::json!({
            "message": {
                "id": msg.id.to_string(),
                "space_id": msg.space_id.to_string(),
                "sender_id": msg.sender_id.to_string(),
                "sender_name": msg.sender_name,
                "msg_type": msg.msg_type,
                "content": msg.content,
                "timestamp": msg.timestamp.to_rfc3339(),
                "status": status_str
            }
        }),
    );
    state.event_bus.broadcast(event).await;

    Json(crate::types::Message {
        id: msg.id,
        space_id: msg.space_id,
        sender_id: msg.sender_id,
        sender_name: msg.sender_name,
        msg_type: if msg_type.as_str() == "image" {
            crate::types::MessageType::Image
        } else {
            crate::types::MessageType::Text
        },
        content: msg.content,
        timestamp: msg.timestamp,
        status,
    })
    .into_response()
}

// ---- 网络 ----
async fn get_network_stats_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    if let Some(rpc_status) = state.easy_tier.get_network_stats(&id).await {
        Json(crate::types::NetworkStats {
            rx_bytes: rpc_status.rx_bytes,
            tx_bytes: rpc_status.tx_bytes,
            rx_packets: 0,
            tx_packets: 0,
            loss_rate: 0.0,
            avg_latency_ms: rpc_status.avg_latency_ms,
        })
        .into_response()
    } else {
        Json(crate::types::NetworkStats {
            rx_bytes: 0,
            tx_bytes: 0,
            rx_packets: 0,
            tx_packets: 0,
            loss_rate: 0.0,
            avg_latency_ms: 0.0,
        })
        .into_response()
    }
}

async fn get_space_peers_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    match state.space_manager.get_peers(&id).await {
        Ok(peers) => Json(peers).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- 日志 ----
async fn get_logs_handler(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let level = params["level"].as_str().map(|s| s.to_string());
    let level_filter = level.and_then(|l| match l.to_lowercase().as_str() {
        "debug" => Some(crate::log::LogLevel::Debug),
        "info" => Some(crate::log::LogLevel::Info),
        "warning" => Some(crate::log::LogLevel::Warning),
        "error" => Some(crate::log::LogLevel::Error),
        _ => None,
    });
    Json(crate::log::get_all(level_filter)).into_response()
}

async fn get_space_logs_handler(
    State(_state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let level = params["level"].as_str().map(|s| s.to_string());
    let level_filter = level.and_then(|l| match l.to_lowercase().as_str() {
        "debug" => Some(crate::log::LogLevel::Debug),
        "info" => Some(crate::log::LogLevel::Info),
        "warning" => Some(crate::log::LogLevel::Warning),
        "error" => Some(crate::log::LogLevel::Error),
        _ => None,
    });
    Json(crate::log::get_by_space(&space_id, level_filter)).into_response()
}

async fn clear_logs_handler() -> impl IntoResponse {
    crate::log::clear();
    StatusCode::NO_CONTENT
}

// ---- v2 复合查询 / 模块发现 / 过滤清除 ----

fn parse_level_opt(s: Option<&str>) -> Option<crate::log::LogLevel> {
    match s.map(|x| x.to_lowercase()).as_deref() {
        Some("debug") => Some(crate::log::LogLevel::Debug),
        Some("info") => Some(crate::log::LogLevel::Info),
        Some("warning") => Some(crate::log::LogLevel::Warning),
        Some("error") => Some(crate::log::LogLevel::Error),
        _ => None,
    }
}

fn parse_category_opt(s: Option<&str>) -> Option<crate::log::LogCategory> {
    match s.map(|x| x.to_lowercase()).as_deref() {
        Some("system") => Some(crate::log::LogCategory::System),
        Some("network") => Some(crate::log::LogCategory::Network),
        Some("webrtc") => Some(crate::log::LogCategory::WebRTC),
        Some("data") => Some(crate::log::LogCategory::Data),
        Some("proxy") => Some(crate::log::LogCategory::Proxy),
        Some("daemon") => Some(crate::log::LogCategory::Daemon),
        Some("space") => Some(crate::log::LogCategory::Space),
        Some("server") => Some(crate::log::LogCategory::Server),
        _ => None,
    }
}

async fn query_logs_handler(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let filter = crate::log::LogFilter {
        level: parse_level_opt(params["level"].as_str()),
        space_id: params["space_id"].as_str().map(|s| s.to_string()),
        module: params["module"].as_str().map(|s| s.to_string()),
        category: parse_category_opt(params["category"].as_str()),
        keyword: params["keyword"].as_str().map(|s| s.to_string()),
        since_seq: params["since_seq"].as_u64(),
        before_ts: params["before_ts"].as_str().map(|s| s.to_string()),
        after_ts: params["after_ts"].as_str().map(|s| s.to_string()),
        limit: params["limit"].as_u64().map(|n| n as usize),
    };
    Json(crate::log::query(&filter)).into_response()
}

async fn get_log_modules_handler() -> impl IntoResponse {
    Json(crate::log::active_modules()).into_response()
}

async fn clear_logs_filtered_handler(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let filter = crate::log::LogFilter {
        level: parse_level_opt(params["level"].as_str()),
        space_id: params["space_id"].as_str().map(|s| s.to_string()),
        module: params["module"].as_str().map(|s| s.to_string()),
        category: parse_category_opt(params["category"].as_str()),
        keyword: params["keyword"].as_str().map(|s| s.to_string()),
        since_seq: None,
        before_ts: None,
        after_ts: None,
        limit: None,
    };
    crate::log::clear_filtered(&filter);
    StatusCode::NO_CONTENT
}

async fn export_logs_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let filter = crate::log::LogFilter {
        level: parse_level_opt(params["level"].as_str()),
        space_id: params["space_id"].as_str().map(|s| s.to_string()),
        module: params["module"].as_str().map(|s| s.to_string()),
        category: parse_category_opt(params["category"].as_str()),
        keyword: params["keyword"].as_str().map(|s| s.to_string()),
        since_seq: None,
        before_ts: params["before_ts"].as_str().map(|s| s.to_string()),
        after_ts: params["after_ts"].as_str().map(|s| s.to_string()),
        limit: None,
    };
    let records = crate::log::query(&filter);
    let format = params["format"].as_str().unwrap_or("txt");
    match crate::log::export_to_dir(&state.data_dir.join("logs_export"), &records, format) {
        Ok(path) => Json(serde_json::json!({ "path": path.to_string_lossy() })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- 配置 ----
async fn get_system_config_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_user_config() {
        Ok(Some(c)) => Json(c).into_response(),
        Ok(None) => Json(serde_json::Value::Null).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn set_system_config_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let config = body["config"].as_str().unwrap_or("").to_string();
    if config.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 config").into_response();
    }
    match state.db.update_user_config(&config) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_app_config_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let kv = state.config.all();
    Json(kv).into_response()
}

async fn set_app_config_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let updates: Vec<(String, String)> = body
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    for (k, v) in &updates {
        if let Err(e) = state.config.set(k, v) {
            return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
        }
    }
    if let Err(e) = state.config.save() {
        return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
    }
    // LOG_ENABLED 立即生效（与桌面端一致：内存标志 + DB）
    if let Some((_, v)) = updates
        .iter()
        .find(|(k, _)| k == crate::config::KEY_LOG_ENABLED)
    {
        let enabled = v != "0";
        crate::log::set_log_enabled(enabled);
        if let Err(e) = state
            .db
            .set_setting("LOG_ENABLED", if enabled { "1" } else { "0" })
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

// ---- 系统信息 ----
async fn get_app_version_handler() -> impl IntoResponse {
    Json(serde_json::json!({ "version": env!("CARGO_PKG_VERSION") })).into_response()
}

async fn check_easytier_binary_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let version = state.easy_tier.get_version().await;
    match version {
        Ok(v) => Json(serde_json::json!({ "present": true, "version": v })).into_response(),
        Err(_) => Json(serde_json::json!({ "present": false })).into_response(),
    }
}

// ---- Proxy ----
async fn get_proxy_url_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let url = state.proxy_server.proxy_url();
    Json(serde_json::json!({ "proxy_url": url })).into_response()
}

async fn get_proxy_status_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let running = state.proxy_server.is_running().await;
    let port = state.proxy_server.port();
    let proxy_url = state.proxy_server.proxy_url();
    Json(serde_json::json!({ "running": running, "port": port, "proxy_url": proxy_url })).into_response()
}

async fn register_proxy_key_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let url = body["url"].as_str().unwrap_or("").to_string();
    if url.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 url").into_response();
    }
    match state.proxy_server.register_proxy_key(&url).await {
        Ok(key) => Json(serde_json::json!({ "key": key })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn set_proxy_source_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let url = body["url"].as_str().unwrap_or("").to_string();
    if url.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 url").into_response();
    }
    state.proxy_server.set_proxy_source(url).await;
    StatusCode::NO_CONTENT.into_response()
}

// ---- Proxy 设备模式 / 下载 ----
async fn set_device_mode_handler(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mode = body["mode"].as_str().unwrap_or("desktop").to_string();
    crate::proxy::hometier_protocol::set_device_mode(&mode);
    StatusCode::NO_CONTENT.into_response()
}

async fn get_pending_downloads_handler(
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let items = crate::proxy::hometier_protocol::pending_downloads()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect::<Vec<_>>();
    Json(serde_json::json!({ "files": items })).into_response()
}

// ---- ACL ----
async fn get_acl_rules_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_acl_rules(&space_id) {
        Ok(rules) => Json(rules).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn create_acl_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let action = body["action"].as_str().unwrap_or("").to_string();
    let source = body["source"].as_str().unwrap_or("").to_string();
    let dest = body["dest"].as_str().unwrap_or("").to_string();
    let ports = body["ports"].as_str().unwrap_or("").to_string();
    let description = body["description"].as_str().unwrap_or("").to_string();
    if action.is_empty() || source.is_empty() || dest.is_empty() || ports.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少必要参数").into_response();
    }
    let now = chrono::Local::now().to_rfc3339();
    let row = crate::db::models::AclRuleRow {
        id: uuid::Uuid::new_v4().to_string(),
        space_id: space_id.clone(),
        action,
        source,
        dest,
        ports,
        description,
        created_at: now.clone(),
        updated_at: now,
    };
    match state.db.insert_acl_rule(&row) {
        Ok(()) => {
            let rule = crate::types::AclRule {
                id: row.id,
                space_id: row.space_id,
                action: row.action,
                source: row.source,
                dest: row.dest,
                ports: row.ports,
                description: row.description,
                created_at: row.created_at,
                updated_at: row.updated_at,
            };
            Json(rule).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn update_acl_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let rule_id = body["rule_id"].as_str().unwrap_or("").to_string();
    let action = body["action"].as_str().map(|s| s.to_string());
    let source = body["source"].as_str().map(|s| s.to_string());
    let dest = body["dest"].as_str().map(|s| s.to_string());
    let ports = body["ports"].as_str().map(|s| s.to_string());
    let description = body["description"].as_str().map(|s| s.to_string());
    if rule_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 rule_id").into_response();
    }
    let existing = match state.db.get_acl_rules(&space_id) {
        Ok(rules) => rules,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    let row = match existing.into_iter().find(|r| r.id == rule_id) {
        Some(r) => r,
        None => return (StatusCode::NOT_FOUND, "ACL 规则不存在").into_response(),
    };
    let updated = crate::db::models::AclRuleRow {
        action: action.unwrap_or(row.action),
        source: source.unwrap_or(row.source),
        dest: dest.unwrap_or(row.dest),
        ports: ports.unwrap_or(row.ports),
        description: description.unwrap_or(row.description),
        updated_at: chrono::Local::now().to_rfc3339(),
        ..row
    };
    match state.db.update_acl_rule(&updated) {
        Ok(()) => {
            let rule = crate::types::AclRule {
                id: updated.id,
                space_id: updated.space_id,
                action: updated.action,
                source: updated.source,
                dest: updated.dest,
                ports: updated.ports,
                description: updated.description,
                created_at: updated.created_at,
                updated_at: updated.updated_at,
            };
            Json(rule).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_acl_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(_space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let rule_id = body["rule_id"].as_str().unwrap_or("").to_string();
    if rule_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 rule_id").into_response();
    }
    match state.db.delete_acl_rule(&rule_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- Port Forward ----
async fn get_port_forward_rules_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_port_forward_rules(&space_id) {
        Ok(rules) => Json(rules).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn create_port_forward_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = body["name"].as_str().unwrap_or("").to_string();
    let protocol = body["protocol"].as_str().unwrap_or("").to_string();
    let source_ip = body["sourceIp"].as_str().unwrap_or("").to_string();
    let source_port = body["sourcePort"].as_i64().unwrap_or(0) as i32;
    let target_ip = body["targetIp"].as_str().unwrap_or("").to_string();
    let target_port = body["targetPort"].as_i64().unwrap_or(0) as i32;
    let description = body["description"].as_str().unwrap_or("").to_string();
    if name.is_empty() || protocol.is_empty() || source_ip.is_empty() || source_port == 0 || target_ip.is_empty() || target_port == 0 {
        return (StatusCode::BAD_REQUEST, "缺少必要参数").into_response();
    }
    let now = chrono::Local::now().to_rfc3339();
    let row = crate::db::models::PortForwardRuleRow {
        id: uuid::Uuid::new_v4().to_string(),
        space_id: space_id.clone(),
        name,
        protocol,
        source_ip,
        source_port,
        target_ip,
        target_port,
        description,
        created_at: now.clone(),
        updated_at: now,
    };
    match state.db.insert_port_forward_rule(&row) {
        Ok(()) => {
            let rule = crate::types::PortForwardRule {
                id: row.id,
                space_id: row.space_id,
                name: row.name,
                protocol: row.protocol,
                source_ip: row.source_ip,
                source_port: row.source_port,
                target_ip: row.target_ip,
                target_port: row.target_port,
                description: row.description,
                created_at: row.created_at,
                updated_at: row.updated_at,
            };
            Json(rule).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn update_port_forward_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let rule_id = body["rule_id"].as_str().unwrap_or("").to_string();
    let name = body["name"].as_str().map(|s| s.to_string());
    let protocol = body["protocol"].as_str().map(|s| s.to_string());
    let source_ip = body["sourceIp"].as_str().map(|s| s.to_string());
    let source_port = body["sourcePort"].as_i64().map(|v| v as i32);
    let target_ip = body["targetIp"].as_str().map(|s| s.to_string());
    let target_port = body["targetPort"].as_i64().map(|v| v as i32);
    let description = body["description"].as_str().map(|s| s.to_string());
    if rule_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 rule_id").into_response();
    }
    let existing = match state.db.get_port_forward_rules(&space_id) {
        Ok(rules) => rules,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    let row = match existing.into_iter().find(|r| r.id == rule_id) {
        Some(r) => r,
        None => return (StatusCode::NOT_FOUND, "端口转发规则不存在").into_response(),
    };
    let updated = crate::db::models::PortForwardRuleRow {
        name: name.unwrap_or(row.name),
        protocol: protocol.unwrap_or(row.protocol),
        source_ip: source_ip.unwrap_or(row.source_ip),
        source_port: source_port.unwrap_or(row.source_port),
        target_ip: target_ip.unwrap_or(row.target_ip),
        target_port: target_port.unwrap_or(row.target_port),
        description: description.unwrap_or(row.description),
        updated_at: chrono::Local::now().to_rfc3339(),
        ..row
    };
    match state.db.update_port_forward_rule(&updated) {
        Ok(()) => {
            let rule = crate::types::PortForwardRule {
                id: updated.id,
                space_id: updated.space_id,
                name: updated.name,
                protocol: updated.protocol,
                source_ip: updated.source_ip,
                source_port: updated.source_port,
                target_ip: updated.target_ip,
                target_port: updated.target_port,
                description: updated.description,
                created_at: updated.created_at,
                updated_at: updated.updated_at,
            };
            Json(rule).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_port_forward_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(_space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let rule_id = body["rule_id"].as_str().unwrap_or("").to_string();
    if rule_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 rule_id").into_response();
    }
    match state.db.delete_port_forward_rule(&rule_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- 应用管理 ----
async fn add_app_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = body["name"].as_str().unwrap_or("").to_string();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 name").into_response();
    }
    if state.space_manager.check_owner(&space_id).await.is_err() {
        return (StatusCode::FORBIDDEN, "仅空间创建者可添加应用").into_response();
    }
    let app = crate::db::models::AppRow {
        id: uuid::Uuid::new_v4().to_string(),
        space_id,
        name,
        category: body["category"].as_str().map(String::from),
        icon: body["icon"].as_str().map(String::from),
        protocol: body["protocol"].as_str().map(String::from).or(Some("http:".to_string())),
        hostname: body["hostname"].as_str().map(String::from),
        port: body["port"].as_str().map(String::from),
        pathname: body["pathname"].as_str().map(String::from),
        sort_order: 0,
        created_by: state.db.get_user_id().ok().flatten().unwrap_or_else(|| "anonymous".to_string()),
        created_at: chrono::Local::now().to_rfc3339(),
    };
    match state.db.insert_app(&app) {
        Ok(()) => Json(app).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn update_app_handler(
    State(state): State<Arc<AppState>>,
    Path(_space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let app_id = body["app_id"].as_str().unwrap_or("").to_string();
    let name = body["name"].as_str().unwrap_or("").to_string();
    if app_id.is_empty() || name.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 app_id 或 name").into_response();
    }
    let caller_id = state.db.get_user_id().ok().flatten().unwrap_or_else(|| "anonymous".to_string());
    let apps = match state.db.list_apps_by_created(&app_id, &caller_id) {
        Ok(a) => a,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    if apps.is_empty() {
        return (StatusCode::FORBIDDEN, "无权限修改或应用不存在").into_response();
    }
    let existing = &apps[0];
    let app = crate::db::models::AppRow {
        id: app_id,
        space_id: existing.space_id.clone(),
        name,
        category: body["category"].as_str().map(String::from),
        icon: body["icon"].as_str().map(String::from),
        protocol: body["protocol"].as_str().map(String::from).or(Some("http:".to_string())),
        hostname: body["hostname"].as_str().map(String::from),
        port: body["port"].as_str().map(String::from),
        pathname: body["pathname"].as_str().map(String::from),
        sort_order: existing.sort_order,
        created_by: caller_id,
        created_at: existing.created_at.clone(),
    };
    match state.db.update_app(&app) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_app_handler(
    State(state): State<Arc<AppState>>,
    Path(_space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let app_id = body["app_id"].as_str().unwrap_or("").to_string();
    if app_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 app_id").into_response();
    }
    let caller_id = state.db.get_user_id().ok().flatten().unwrap_or_else(|| "anonymous".to_string());
    match state.db.delete_app(&app_id, &caller_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn list_apps_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
) -> impl IntoResponse {
    match state.db.list_apps(&space_id) {
        Ok(apps) => Json(apps).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_system_apps_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    Json(crate::system_apps::load_system_apps(&state.data_dir)).into_response()
}

async fn share_app_handler(
    State(state): State<Arc<AppState>>,
    Path(_space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let app_id = body["app_id"].as_str().unwrap_or("").to_string();
    let target_space_id = body["target_space_id"].as_str().unwrap_or("").to_string();
    if app_id.is_empty() || target_space_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 app_id/target_space_id").into_response();
    }
    let caller_id = state
        .db
        .get_user_id()
        .ok()
        .flatten()
        .unwrap_or_else(|| "anonymous".to_string());
    let apps = match state.db.list_apps_by_created(&app_id, &caller_id) {
        Ok(a) => a,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    if apps.is_empty() {
        return (StatusCode::FORBIDDEN, "无权限分享或应用不存在").into_response();
    }
    let source = &apps[0];
    // 目标空间必须存在
    let spaces = match state.space_manager.list().await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    if !spaces.iter().any(|s| s.id.to_string() == target_space_id) {
        return (StatusCode::NOT_FOUND, "目标空间不存在").into_response();
    }
    // 目标空间已存在同名应用则跳过
    if let Ok(existing) = state.db.list_apps(&target_space_id) {
        if existing.iter().any(|a| a.name == source.name) {
            return (StatusCode::CONFLICT, "目标空间已存在同名应用").into_response();
        }
    }
    let app = crate::db::models::AppRow {
        id: uuid::Uuid::new_v4().to_string(),
        space_id: target_space_id,
        name: source.name.clone(),
        category: source.category.clone(),
        icon: source.icon.clone(),
        protocol: source.protocol.clone(),
        hostname: source.hostname.clone(),
        port: source.port.clone(),
        pathname: source.pathname.clone(),
        sort_order: source.sort_order,
        created_by: caller_id,
        created_at: chrono::Local::now().to_rfc3339(),
    };
    match state.db.insert_app(&app) {
        Ok(()) => Json(app).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- File Transfer (Web 模式：服务器中转) ----

/// POST /file/send?space_id=..&file_name=..&password=..
/// body = 原始文件字节。服务器保存本地副本（供 Web 下载），
/// 并作为 EasyTier 节点 P2P 转发给所有在线成员。
async fn send_file_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let space_id = params["space_id"].as_str().unwrap_or("").to_string();
    let file_name = params["file_name"].as_str().unwrap_or("").to_string();
    if space_id.is_empty() || file_name.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 space_id/file_name").into_response();
    }
    let password = params["password"].as_str().map(|s| s.to_string());
    let space_uuid = match uuid::Uuid::parse_str(&space_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "space_id 格式错误").into_response(),
    };
    let sender_id = space_uuid;

    let peers = match state.space_manager.get_peers_for_file_transfer(&space_uuid).await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    // 写入临时文件（storage_dir/{file_id}.tmp），供 P2P 发送读取
    let file_id = uuid::Uuid::new_v4();
    let tmp_path = state.file_registry.storage_dir().join(format!("{}.tmp", file_id));
    if let Err(e) = std::fs::write(&tmp_path, &body) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("写入临时文件失败: {}", e)).into_response();
    }

    // 服务器本地保存原始副本（Web 端下载用）
    let file_server = match state.file_registry.get_or_start(&space_uuid).await {
        Ok(fs) => fs,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
        }
    };
    if let Err(e) = file_server.write_file(&file_id, &body).await {
        let _ = std::fs::remove_file(&tmp_path);
        return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
    }

    // 离线场景：无在线 peer 时仅服务器存储，接收方上线后通过信令 + HTTP 下载
    let file_info = if peers.is_empty() {
        let fi = FileInfo {
            id: file_id,
            space_id: space_uuid,
            sender_id,
            file_name: file_name.clone(),
            file_size: body.len() as u64,
            file_hash: Some(crate::crypto::sha256_hex(&body)),
            mime_type: None,
            is_compressed: false,
            is_password_protected: password.is_some(),
            storage_path: None,
            created_at: chrono::Local::now(),
        };
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let _ = std::fs::remove_file(&tmp_path);
        fi
    } else {
        // P2P 发送给所有在线成员（复用同一 file_id）
        let mut last_file_info = None;
        let mut shared_file_id: Option<uuid::Uuid> = Some(file_id);
        for (target_ip, target_port) in &peers {
            match state
                .file_manager
                .send_file(
                    space_uuid,
                    sender_id,
                    tmp_path.clone(),
                    password.clone(),
                    target_ip,
                    *target_port,
                    shared_file_id,
                )
                .await
            {
                Ok(fi) => {
                    shared_file_id = Some(fi.id);
                    last_file_info = Some(fi);
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp_path);
                    return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
                }
            }
        }
        let _ = std::fs::remove_file(&tmp_path);

        match last_file_info {
            Some(fi) => fi,
            None => return (StatusCode::INTERNAL_SERVER_ERROR, "文件发送失败").into_response(),
        }
    };

    // 保存到数据库
    let row = crate::db::models::FileRow {
        id: file_info.id.to_string(),
        space_id: file_info.space_id.to_string(),
        sender_id: file_info.sender_id.to_string(),
        file_name: file_info.file_name.clone(),
        file_size: file_info.file_size as i64,
        file_hash: file_info.file_hash.clone(),
        mime_type: file_info.mime_type.clone(),
        is_compressed: file_info.is_compressed,
        is_password_protected: file_info.is_password_protected,
        storage_path: file_info.storage_path.clone(),
        created_at: file_info.created_at.to_rfc3339(),
    };
    if let Err(e) = state.db.insert_file(&row) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
    }

    Json(serde_json::json!({
        "transfer_id": file_info.id.to_string(),
        "file_info": {
            "id": file_info.id.to_string(),
            "space_id": file_info.space_id.to_string(),
            "sender_id": file_info.sender_id.to_string(),
            "file_name": file_info.file_name,
            "file_size": file_info.file_size,
            "file_hash": file_info.file_hash,
            "mime_type": file_info.mime_type,
            "is_compressed": file_info.is_compressed,
            "is_password_protected": file_info.is_password_protected,
            "storage_path": file_info.storage_path,
            "created_at": file_info.created_at.to_rfc3339(),
        }
    }))
    .into_response()
}

/// GET /file/{space_id}/download/{file_id}
/// 返回服务器本地保存的文件字节（Web 端浏览器直接下载）
async fn receive_file_handler(
    State(state): State<Arc<AppState>>,
    Path((space_id, file_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let space_uuid = match uuid::Uuid::parse_str(&space_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "space_id 格式错误").into_response(),
    };
    let file_uuid = match uuid::Uuid::parse_str(&file_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "file_id 格式错误").into_response(),
    };

    let file_name = state
        .db
        .get_file(&space_id, &file_id)
        .ok()
        .and_then(|opt| opt.map(|f| f.file_name))
        .unwrap_or_else(|| file_id.clone());

    let file_server = match state.file_registry.get_or_start(&space_uuid).await {
        Ok(fs) => fs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    let data = match file_server.read_file(&file_uuid).await {
        Ok(d) => d,
        Err(e) => return (StatusCode::NOT_FOUND, e).into_response(),
    };

    let safe_name: String = file_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let disposition = format!("attachment; filename=\"{}\"", safe_name);
    ([
        (axum::http::header::CONTENT_TYPE, "application/octet-stream".to_string()),
        (axum::http::header::CONTENT_DISPOSITION, disposition),
    ], data)
        .into_response()
}

async fn record_received_file_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let row = crate::db::models::FileRow {
        id: body["id"].as_str().unwrap_or("").to_string(),
        space_id: body["space_id"].as_str().unwrap_or("").to_string(),
        sender_id: body["sender_id"].as_str().unwrap_or("").to_string(),
        file_name: body["file_name"].as_str().unwrap_or("").to_string(),
        file_size: body["file_size"].as_i64().unwrap_or(0),
        file_hash: body["file_hash"].as_str().map(|s| s.to_string()),
        mime_type: body["mime_type"].as_str().map(|s| s.to_string()),
        is_compressed: body["is_compressed"].as_bool().unwrap_or(false),
        is_password_protected: body["is_password_protected"].as_bool().unwrap_or(false),
        storage_path: body["storage_path"].as_str().map(|s| s.to_string()),
        created_at: body["created_at"].as_str().unwrap_or("").to_string(),
    };
    if row.id.is_empty() || row.space_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 id/space_id").into_response();
    }
    match state.db.insert_file(&row) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_file_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let space_id = body["space_id"].as_str().unwrap_or("").to_string();
    let file_id = body["file_id"].as_str().unwrap_or("").to_string();
    if space_id.is_empty() || file_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 space_id/file_id").into_response();
    }
    match state.db.delete_file(&space_id, &file_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn list_files_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let limit = params["limit"].as_u64().map(|v| v as u32);
    match state.space_manager.list_space_files(&space_id, limit).await {
        Ok(rows) => {
            let files: Vec<serde_json::Value> = rows.iter().map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "space_id": r.space_id,
                    "sender_id": r.sender_id,
                    "file_name": r.file_name,
                    "file_size": r.file_size,
                    "file_hash": r.file_hash,
                    "mime_type": r.mime_type,
                    "is_compressed": r.is_compressed,
                    "is_password_protected": r.is_password_protected,
                    "storage_path": r.storage_path,
                    "created_at": r.created_at,
                })
            }).collect();
            Json(files).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_transfer_progress_handler(
    State(_state): State<Arc<AppState>>,
    Query(_params): Query<serde_json::Value>,
) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "P2P 传输进度查询，Web 模式暂不支持").into_response()
}

// ---- Space Signal ----
async fn send_signal_handler(
    State(state): State<Arc<AppState>>,
    Path(space_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let payload = body["payload"].as_str().unwrap_or("").to_string();
    let target = body["target"].as_str().map(|s| s.to_string());
    if payload.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 payload").into_response();
    }
    let id = match parse_uuid(&space_id).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    let sender_id = machine_id::get_machine_id()
        .map(|s| Uuid::parse_str(&s).unwrap_or(Uuid::nil()))
        .unwrap_or(Uuid::nil());
    let sender_name = gethostname::gethostname().to_string_lossy().to_string();
    let space = state.space_manager.spaces.read().await
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| "Space not found".to_string());
    let space = match space {
        Ok(s) => s,
        Err(e) => return (StatusCode::NOT_FOUND, e).into_response(),
    };
    let mut msg = ChatMessage::signal(id, sender_id, sender_name, payload);
    msg.sign(&space.network_secret);
    match state.space_manager.send_signal_to(&id, target.as_deref().unwrap_or(""), &msg).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- EasyTier Upgrade ----
async fn check_easytier_update_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.easy_tier.downloader.check_update("latest").await {
        Ok(has_update) => Json(serde_json::json!({ "has_update": has_update })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn upgrade_easytier_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let version = body["version"].as_str().unwrap_or("").to_string();
    let _use_proxy = body["useProxy"].as_bool().unwrap_or(false);
    if version.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 version").into_response();
    }
    match state.easy_tier.upgrade(&version, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- 应用更新 ----
async fn check_app_update_handler() -> impl IntoResponse {
    Json(crate::commands::update_app::check_app_update().await).into_response()
}

async fn upgrade_app_handler(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let use_proxy = body["useProxy"].as_bool().unwrap_or(false);
    match crate::commands::update_app::upgrade_app_inner(use_proxy, |_| {}).await {
        Ok(outcome) => Json(outcome).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- Config Paths ----
async fn get_config_path_handler() -> impl IntoResponse {
    match crate::commands::config::get_config_file_path() {
        Ok(path) => Json(serde_json::json!({ "path": path })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
async fn get_config_template_path_handler() -> impl IntoResponse {
    match crate::commands::config::get_config_template_path() {
        Ok(path) => Json(serde_json::json!({ "path": path })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ---- Config Store（P2P 分布式配置同步）----
async fn get_config_version_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.config_store.store.get_meta(&name) {
        Some(meta) => Json(meta).into_response(),
        None => (StatusCode::NOT_FOUND, "配置不存在").into_response(),
    }
}

async fn download_config_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.config_store.store.get_file(&name) {
        Ok(Some(file)) => Json(file).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "配置不存在").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn upload_config_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = body["name"].as_str().unwrap_or("").to_string();
    let version = body["version"].as_u64().unwrap_or(0) as u32;
    let timestamp = body["timestamp"].as_u64().unwrap_or(0);
    let content = body["content"]
        .as_str()
        .map(|s| s.as_bytes().to_vec())
        .unwrap_or_default();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 name").into_response();
    }
    let file = crate::config_store::ConfigFile {
        name,
        version,
        content,
        timestamp,
        checksum: None,
    };
    state.config_store.store_local(file);
    StatusCode::NO_CONTENT.into_response()
}

async fn get_remote_config_version_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let ip = params.get("ip").cloned().unwrap_or_default();
    let name = params.get("name").cloned().unwrap_or_default();
    if ip.is_empty() || name.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 ip/name").into_response();
    }
    let remote = crate::config_store::client::RemoteStore::new(
        &ip,
        crate::config_store::DEFAULT_PORT,
    );
    match remote.query_version(&name).await {
        Ok(Some(meta)) => Json(meta).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "远端配置不存在").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn download_remote_config_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let ip = params.get("ip").cloned().unwrap_or_default();
    let name = params.get("name").cloned().unwrap_or_default();
    if ip.is_empty() || name.is_empty() {
        return (StatusCode::BAD_REQUEST, "缺少 ip/name").into_response();
    }
    let remote = crate::config_store::client::RemoteStore::new(
        &ip,
        crate::config_store::DEFAULT_PORT,
    );
    match remote.request_file(&name).await {
        Ok(Some(file)) => Json(file).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "远端配置不存在").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

pub fn static_file_handler(static_dir: String) -> axum::Router {
    use axum::routing::any;

    // 优先使用嵌入式 dist（编译时嵌入，无运行时依赖）；
    // 若 dist/ 不存在于嵌入中，回退到 ServeDir 文件系统。
    if std::path::Path::new(&static_dir).exists() {
        Router::new().fallback_service(ServeDir::new(static_dir))
    } else {
        Router::new().fallback(any(|uri: axum::http::Uri| async move {
            crate::server::assets::serve_embedded(uri)
        }))
    }
}