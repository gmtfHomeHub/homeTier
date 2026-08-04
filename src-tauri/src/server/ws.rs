use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::server::AppState;
use crate::server::event::{EventType, ServerEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    // 客户端 -> 服务端
    Subscribe { space_id: String },
    Unsubscribe { space_id: String },
    Signal { space_id: String, payload: serde_json::Value, target: Option<String> },
    Heartbeat,
    // 服务端 -> 客户端
    Event { event: ServerEvent },
    Connected { space_id: String },
    Error { message: String },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(space_id): Path<String>,
    State(app_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, space_id, app_state))
}

async fn handle_socket(socket: WebSocket, space_id: String, app_state: Arc<AppState>) {
    let event_bus = app_state.event_bus.clone();
    
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    
    // 发送连接确认
    {
        let mut s = sender.lock().await;
        let _ = s.send(Message::Text(serde_json::to_string(&WsMessage::Connected {
            space_id: space_id.clone(),
        }).unwrap().into())).await;
    }

    // 订阅共享事件总线
    let mut rx = event_bus.subscribe().await;
    
    // 向客户端发送事件的任务
    let send_sender = sender.clone();
    let sender_task = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if event.space_id.as_deref() == Some(&space_id) || event.space_id.is_none() {
                if let Ok(text) = serde_json::to_string(&WsMessage::Event { event }) {
                    let mut s = send_sender.lock().await;
                    if s.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // 接收客户端消息的任务
    let receiver_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                        match ws_msg {
                            WsMessage::Subscribe { space_id: _sub_id } => {
                                // 处理订阅逻辑（如需要可扩展）
                            }
                            WsMessage::Signal { space_id: sig_space, payload, target } => {
                                // 转发信令给目标用户或广播
                                let event = ServerEvent::new(
                                    EventType::Custom("signal".into()),
                                    Some(sig_space),
                                    serde_json::json!({ "payload": payload, "target": target }),
                                );
                                event_bus.broadcast(event).await;
                            }
                            WsMessage::Heartbeat => {
                                let mut s = sender.lock().await;
                                let _ = s.send(Message::Text("pong".to_string().into())).await;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    });

    // 等待任一任务结束
    tokio::select! {
        _ = sender_task => {},
        _ = receiver_task => {},
    }
}