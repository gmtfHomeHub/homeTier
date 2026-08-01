use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 聊天消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub space_id: Uuid,
    pub sender_id: Uuid,
    pub sender_name: String,
    pub msg_type: String, // "text", "image", "system"
    pub content: String,
    pub timestamp: DateTime<Local>,
    pub signature: Option<String>,
}

impl ChatMessage {
    /// 创建文本消息
    pub fn text(space_id: Uuid, sender_id: Uuid, sender_name: String, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            space_id,
            sender_id,
            sender_name,
            msg_type: "text".into(),
            content,
            timestamp: Local::now(),
            signature: None,
        }
    }

    /// 创建图片消息
    pub fn image(
        space_id: Uuid,
        sender_id: Uuid,
        sender_name: String,
        base64_content: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            space_id,
            sender_id,
            sender_name,
            msg_type: "image".into(),
            content: base64_content,
            timestamp: Local::now(),
            signature: None,
        }
    }

    /// 创建信令消息（WebRTC offer/answer/ice 等，不落库）
    pub fn signal(space_id: Uuid, sender_id: Uuid, sender_name: String, payload: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            space_id,
            sender_id,
            sender_name,
            msg_type: "signal".into(),
            content: payload,
            timestamp: Local::now(),
            signature: None,
        }
    }

    /// 创建系统消息
    pub fn system(space_id: Uuid, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            space_id,
            sender_id: Uuid::nil(),
            sender_name: "System".into(),
            msg_type: "system".into(),
            content,
            timestamp: Local::now(),
            signature: None,
        }
    }

    /// 对消息签名
    pub fn sign(&mut self, secret: &str) {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let data = format!(
            "{}{}{}{}",
            self.id, self.sender_id, self.content, self.timestamp
        );
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(data.as_bytes());
        self.signature = Some(hex::encode(mac.finalize().into_bytes()));
    }

    /// 验证消息签名
    pub fn verify(&self, secret: &str) -> bool {
        if let Some(ref sig) = self.signature {
            use hmac::{Hmac, Mac};
            use sha2::Sha256;
            type HmacSha256 = Hmac<Sha256>;

            let data = format!(
                "{}{}{}{}",
                self.id, self.sender_id, self.content, self.timestamp
            );
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
            mac.update(data.as_bytes());
            let expected = hex::encode(mac.finalize().into_bytes());
            sig == &expected
        } else {
            false
        }
    }
}
