use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 语音频道状态
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceStatus {
    Disconnected,
    Connecting,
    Connected,
    Muted,
}

/// WebRTC 语音引擎
#[derive(Clone)]
pub struct VoiceEngine {
    pub space_id: String,
    pub status: Arc<RwLock<VoiceStatus>>,
    pub mic_muted: Arc<RwLock<bool>>,
    pub speaker_muted: Arc<RwLock<bool>>,
    peers: Arc<RwLock<HashMap<String, WebRtcPeer>>>,
}

#[derive(Clone)]
struct WebRtcPeer {
    peer_id: String,
    connected: bool,
}

impl VoiceEngine {
    pub fn new(space_id: String) -> Self {
        Self {
            space_id,
            status: Arc::new(RwLock::new(VoiceStatus::Disconnected)),
            mic_muted: Arc::new(RwLock::new(false)),
            speaker_muted: Arc::new(RwLock::new(false)),
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 加入语音频道
    pub async fn join(&self) -> Result<(), String> {
        *self.status.write().await = VoiceStatus::Connecting;

        // 通过信令服务获取在线成员列表并建立 WebRTC 连接
        // 这里使用信令通道交换 SDP Offer/Answer
        // 实际实现时会通过 EasyTier 虚拟网络的信令通道建立 P2P 连接
        let peers = self.peers.clone();
        let status = self.status.clone();

        // 模拟信令连接过程
        tokio::spawn(async move {
            // 连接信令服务器，获取 peer 列表
            // 对每个 peer 创建 RTCPeerConnection
            // 交换 SDP Offer/Answer
            // 建立 ICE 连接
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            *status.write().await = VoiceStatus::Connected;
        });

        // 等待连接建立
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        if *self.status.read().await != VoiceStatus::Connected {
            *self.status.write().await = VoiceStatus::Connected;
        }

        Ok(())
    }

    /// 离开语音频道
    pub async fn leave(&self) -> Result<(), String> {
        // 关闭所有 WebRTC PeerConnection
        let mut peers = self.peers.write().await;
        for (_, peer) in peers.iter_mut() {
            peer.connected = false;
            // 实际会调用 peer.close() 关闭连接
        }
        peers.clear();

        *self.status.write().await = VoiceStatus::Disconnected;
        Ok(())
    }

    /// 添加 WebRTC peer
    pub async fn add_peer(&self, peer_id: String) {
        let mut peers = self.peers.write().await;
        peers.insert(peer_id.clone(), WebRtcPeer {
            peer_id: peer_id.clone(),
            connected: true,
        });
    }

    /// 移除 WebRTC peer
    pub async fn remove_peer(&self, peer_id: &str) {
        let mut peers = self.peers.write().await;
        peers.remove(peer_id);
    }

    /// 切换麦克风
    pub async fn toggle_mic(&self) -> bool {
        let mut muted = self.mic_muted.write().await;
        *muted = !*muted;
        *muted
    }

    /// 切换扬声器
    pub async fn toggle_speaker(&self) -> bool {
        let mut muted = self.speaker_muted.write().await;
        *muted = !*muted;
        *muted
    }

    pub async fn get_status(&self) -> VoiceStatus {
        self.status.read().await.clone()
    }

    pub async fn is_mic_muted(&self) -> bool {
        *self.mic_muted.read().await
    }

    pub async fn is_speaker_muted(&self) -> bool {
        *self.speaker_muted.read().await
    }
}

/// 语音频道管理器
pub struct VoiceManager {
    channels: dashmap::DashMap<String, VoiceEngine>,
}

impl VoiceManager {
    pub fn new() -> Self {
        Self { channels: dashmap::DashMap::new() }
    }

    pub fn get(&self, space_id: &str) -> Option<VoiceEngine> {
        self.channels.get(space_id).map(|e| e.clone())
    }

    pub fn get_or_create(&self, space_id: &str) -> VoiceEngine {
        if let Some(engine) = self.channels.get(space_id) {
            return engine.clone();
        }
        let engine = VoiceEngine::new(space_id.to_string());
        self.channels.insert(space_id.to_string(), engine.clone());
        engine
    }

    pub fn remove(&self, space_id: &str) {
        self.channels.remove(space_id);
    }
}