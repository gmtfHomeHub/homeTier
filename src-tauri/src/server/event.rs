use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    SpaceCreated,
    SpaceDeleted,
    SpaceUpdated,
    MemberJoined,
    MemberLeft,
    MessageSent,
    FileShared,
    ScreenShareStarted,
    ScreenShareStopped,
    VoiceCallStarted,
    VoiceCallStopped,
    PeerConnected,
    PeerDisconnected,
    ConfigChanged,
    SystemLog,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEvent {
    pub event_type: EventType,
    pub space_id: Option<String>,
    pub payload: serde_json::Value,
    pub timestamp: i64,
}

impl ServerEvent {
    pub fn new(event_type: EventType, space_id: Option<String>, payload: serde_json::Value) -> Self {
        Self {
            event_type,
            space_id,
            payload,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

pub struct GlobalEventBus {
    pub subscribers: Arc<RwLock<Vec<tokio::sync::mpsc::UnboundedSender<ServerEvent>>>>,
    pub recent_events: Arc<RwLock<VecDeque<ServerEvent>>>,
    pub max_recent: usize,
}

impl GlobalEventBus {
    pub fn new(max_recent: usize) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
            recent_events: Arc::new(RwLock::new(VecDeque::with_capacity(max_recent))),
            max_recent,
        }
    }

    pub async fn subscribe(&self) -> tokio::sync::mpsc::UnboundedReceiver<ServerEvent> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.subscribers.write().await.push(tx);
        rx
    }

    pub async fn broadcast(&self, event: ServerEvent) {
        // 存储最近的事件
        {
            let mut recent = self.recent_events.write().await;
            recent.push_back(event.clone());
            if recent.len() > self.max_recent {
                recent.pop_front();
            }
        }

        // 广播给所有订阅者
        let mut subscribers = self.subscribers.write().await;
        subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }

    pub async fn get_recent(&self, limit: Option<usize>) -> Vec<ServerEvent> {
        let recent = self.recent_events.read().await;
        let limit = limit.unwrap_or(self.max_recent);
        recent.iter().rev().take(limit).cloned().collect()
    }

    pub async fn get_recent_for_space(&self, space_id: &str, limit: Option<usize>) -> Vec<ServerEvent> {
        let recent = self.recent_events.read().await;
        let limit = limit.unwrap_or(self.max_recent);
        recent
            .iter()
            .filter(|e| e.space_id.as_deref() == Some(space_id))
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
}