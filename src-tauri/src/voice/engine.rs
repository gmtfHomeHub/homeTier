use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;

/// 语音频道状态
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceStatus {
    Disconnected,
    Connecting,
    Connected,
    Muted,
}

/// WebRTC 语音引擎
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

        // 启动信令服务器（TODO: 实现 VoiceServer）
        // let mut signal_server = VoiceServer::new(self.signal_port);
        // signal_server.start().await.map_err(|e| format!("启动信令服务器失败: {}", e))?;

        // 获取 peer 列表
        let peers = self.peers.clone();
        let status = self.status.clone();
        let space_id = self.space_id.clone();
        let signal_port = self.signal_port;

        // 建立 WebRTC 连接
        tokio::spawn(async move {
            // 创建 WebRTC PeerConnection
            let api = APIBuilder::new().build();
            let config = RTCConfiguration::default();
            let peer_connection = api
                .new_peer_connection(config)
                .await
                .map_err(|e| format!("创建 PeerConnection 失败: {}", e))
                .unwrap();

            // 创建 SDP Offer
            let offer = peer_connection
                .create_offer(None)
                .await
                .map_err(|e| format!("创建 Offer 失败: {}", e))
                .unwrap();

            // 设置本地描述
            peer_connection
                .set_local_description(offer.clone())
                .await
                .map_err(|e| format!("设置本地描述失败: {}", e))
                .unwrap();

            // 发送 Offer 到信令服务器（TODO: 实现 SignalHandler）
            // let _ = SignalHandler::send_offer("127.0.0.1", signal_port, &offer.sdp).await;

            // 设置远程描述（这里需要从信令服务器获取）
            // 在实际实现中，会从信令服务器获取远程 peer 的 SDP Answer
            // 并设置到 peer_connection 中

            // 标记为已连接
            *status.write().await = VoiceStatus::Connected;
        });

        // 等待连接建立
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

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
        None // TODO: 重新实现 VoiceManager::get（VoiceEngine 不再 Clone）
    }

    pub fn get_or_create(&self, space_id: &str) -> VoiceEngine {
        VoiceEngine::new(space_id.to_string())
    }

    pub fn remove(&self, space_id: &str) {
        self.channels.remove(space_id);
    }
}