use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use crate::voice::server::VoiceServer;

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
    /// 信令服务器端口
    signal_port: u16,
}

#[derive(Clone)]
struct WebRtcPeer {
    peer_id: String,
    connected: bool,
}

impl VoiceEngine {
    pub fn new(space_id: String) -> Self {
        let signal_port = 18100 + (space_id.parse::<u128>().unwrap_or(0) % 100) as u16;
        Self {
            space_id,
            status: Arc::new(RwLock::new(VoiceStatus::Disconnected)),
            mic_muted: Arc::new(RwLock::new(false)),
            speaker_muted: Arc::new(RwLock::new(false)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            signal_port,
        }
    }

    /// 加入语音频道
    pub async fn join(&self) -> Result<(), String> {
        *self.status.write().await = VoiceStatus::Connecting;

        // 启动信令服务器
        let mut signal_server = VoiceServer::new(self.signal_port);
        signal_server.start().await.map_err(|e| format!("启动信令服务器失败: {}", e))?;

        // 获取 peer 列表
        let peers = self.peers.clone();
        let status = self.status.clone();
        let space_id = self.space_id.clone();

        // 建立 WebRTC 连接
        tokio::spawn(async move {
            let api = APIBuilder::new().build();
            let config = RTCConfiguration::default();
            let peer_connection = match api.new_peer_connection(config).await {
                Ok(pc) => pc,
                Err(e) => {
                    crate::log_error!(format!("创建 PeerConnection 失败: {}", e));
                    *status.write().await = VoiceStatus::Disconnected;
                    return;
                }
            };

            let offer = match peer_connection.create_offer(None).await {
                Ok(o) => o,
                Err(e) => {
                    crate::log_error!(format!("创建 Offer 失败: {}", e));
                    *status.write().await = VoiceStatus::Disconnected;
                    return;
                }
            };

            if let Err(e) = peer_connection.set_local_description(offer.clone()).await {
                crate::log_error!(format!("设置本地描述失败: {}", e));
                *status.write().await = VoiceStatus::Disconnected;
                return;
            }

            crate::log_info!(format!("语音频道已加入: space_id={}", space_id));
            *status.write().await = VoiceStatus::Connected;
        });

        Ok(())
    }

    /// 离开语音频道
    pub async fn leave(&self) -> Result<(), String> {
        crate::log_info!(format!("离开语音频道: space_id={}", self.space_id));
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
        crate::log_info!(format!("添加语音 peer: space_id={}, peer_id={}", self.space_id, peer_id));
        let mut peers = self.peers.write().await;
        peers.insert(peer_id.clone(), WebRtcPeer {
            peer_id: peer_id.clone(),
            connected: true,
        });
    }

    /// 移除 WebRTC peer
    pub async fn remove_peer(&self, peer_id: &str) {
        crate::log_info!(format!("移除语音 peer: space_id={}, peer_id={}", self.space_id, peer_id));
        let mut peers = self.peers.write().await;
        peers.remove(peer_id);
    }

    /// 切换麦克风
    pub async fn toggle_mic(&self) -> bool {
        let mut muted = self.mic_muted.write().await;
        *muted = !*muted;
        crate::log_info!(format!("切换麦克风: space_id={}, muted={}", self.space_id, *muted));
        *muted
    }

    /// 切换扬声器
    pub async fn toggle_speaker(&self) -> bool {
        let mut muted = self.speaker_muted.write().await;
        *muted = !*muted;
        crate::log_info!(format!("切换扬声器: space_id={}, muted={}", self.space_id, *muted));
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
        self.channels.get(space_id).map(|c| c.value().clone())
    }

    pub fn get_or_create(&self, space_id: &str) -> VoiceEngine {
        self.channels
            .entry(space_id.to_string())
            .or_insert_with(|| VoiceEngine::new(space_id.to_string()))
            .value()
            .clone()
    }

    pub fn remove(&self, space_id: &str) {
        self.channels.remove(space_id);
    }
}