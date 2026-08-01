use std::collections::HashMap;
use reqwest::Client;
use crate::chat::message::ChatMessage;

/// P2P 聊天客户端，用于发送消息到其他节点
pub struct ChatClient {
    client: Client,
    /// 节点地址映射: member_id -> (ip, port)
    peers: HashMap<String, (String, u16)>,
}

impl ChatClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            peers: HashMap::new(),
        }
    }

    /// 更新对等节点列表
    pub fn update_peers(&mut self, peers: HashMap<String, (String, u16)>) {
        self.peers = peers;
    }

    /// 获取对等节点数量
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// 广播消息到所有对等节点
    pub async fn broadcast(&self, msg: &ChatMessage) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        for (member_id, (ip, port)) in &self.peers {
            let url = format!("http://{}:{}/message", ip, port);
            let body = match serde_json::to_string(msg) {
                Ok(b) => b,
                Err(e) => {
                    errors.push((member_id.clone(), e.to_string()));
                    continue;
                }
            };

            match self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    errors.push((member_id.clone(), e.to_string()));
                }
            }
        }
        errors
    }

    /// 发送消息到指定节点
    pub async fn send_to(&self, member_id: &str, msg: &ChatMessage) -> Result<(), String> {
        let (ip, port) = self.peers.get(member_id)
            .ok_or_else(|| format!("Peer {} not found", member_id))?;
        let url = format!("http://{}:{}/message", ip, port);
        let body = serde_json::to_string(msg).map_err(|e| e.to_string())?;

        self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Send error: {}", e))?;

        Ok(())
    }
}