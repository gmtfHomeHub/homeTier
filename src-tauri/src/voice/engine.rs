use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;

use crate::voice::server::VoiceServer;
use crate::voice::signal::{SignalHandler, poll_signal, SignalPath};

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
    signal_port: u16,
    server_handle: Arc<RwLock<Option<VoiceServer>>>,
}

#[derive(Clone)]
struct WebRtcPeer {
    peer_id: String,
    pc: Option<Arc<RTCPeerConnection>>,
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
            server_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// 加凡语音频道
    pub async fn join(&self) -> Result<(), String> {
        *self.status.write().await = VoiceStatus::Connecting;

        // 启动信令服务器
        let mut signal_server = VoiceServer::new(self.signal_port);
        signal_server.start().await.map_err(|e| format!("启动信令服务器失败: {}", e))?;
        *self.server_handle.write().await = Some(signal_server);

        let status = self.status.clone();
        let space_id = self.space_id.clone();
        let signal_port = self.signal_port;

        tokio::spawn(async move {
            let api = APIBuilder::new().build();
            let config = RTCConfiguration::default();

            let peer_connection = match api.new_peer_connection(config).await {
                Ok(pc) => Arc::new(pc),
                Err(e) => {
                    crate::log_error!(format!("创建 PeerConnection 失败: {}", e));
                    *status.write().await = VoiceStatus::Disconnected;
                    return;
                }
            };

            // 注册 ICE 候选回调
            let pc_ice = peer_connection.clone();
            peer_connection.on_ice_candidate(Box::new(move |c: Option<RTCIceCandidate>| {
                if let Some(candidate) = c {
                    let cand_json = candidate.to_json().unwrap_or_default();
                    let cand_str = serde_json::to_string(&cand_json).unwrap_or_default();
                    let signal_port = signal_port;
                    tokio::spawn(async move {
                        let _ = SignalHandler::send_ice(
                            "127.0.0.1",
                            signal_port,
                            &cand_str,
                        ).await;
                    });
                }
                Box::pin(async {})
            }));

            // 注册来放状态状态理
            let status_conn = status.clone();
            peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
                let status = status_conn.clone();
                tokio::spawn(async move {
                    match s {
                        RTCPeerConnectionState::Failed |
                        RTCPeerConnectionState::Disconnected => {
                            *status.write().await = VoiceStatus::Disconnected;
                        }
                        _ => {}
                    }
                });
                Box::pin(async {})
            }));

            // 添加音频轨道
            let audio_track: Arc<dyn TrackLocal + Send + Sync> = Arc::new(TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    mime_type: "audio/opus".to_string(),
                    clock_rate: 48000,
                    channels: 2,
                    sdp_fmtp_line: "".to_string(),
                    rtcp_feedback: vec![],
                },
                "audio".to_string(),
                "voice".to_string(),
            ));

            if let Err(e) = peer_connection.add_track(Arc::clone(&audio_track)).await {
                crate::log_error!(format!("添加音频轨道失败: {}", e));
                *status.write().await = VoiceStatus::Disconnected;
                return;
            }

            // 创建 Offer
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

            crate::log_info!(format!("voice offer created: space={}, port={}", space_id, signal_port));

            // 后台消息轮询
            let pc_poll = peer_connection.clone();
            let status_poll = status.clone();
            let space_id_poll = space_id.clone();

            tokio::spawn(async move {
                loop {
                    if let Some(answer_msg) = poll_signal(
                        |m| matches!(m.path, SignalPath::Answer),
                        2000,
                        &space_id_poll,
                    ).await {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&answer_msg.body) {
                                let sdp = json.get("sdp").and_then(|v| v.as_str()).unwrap_or("");
                                let desc = RTCSessionDescription::answer(sdp.to_string()).unwrap();
                                let _ = pc_poll.set_remote_description(desc).await;
                                *status_poll.write().await = VoiceStatus::Connected;
                            }
                    }

                    // 批量处理 ICE
                    while let Some(ice) = poll_signal(
                        |m| matches!(m.path, SignalPath::Ice),
                        100,
                        &space_id_poll,
                    ).await {
                        let pc = pc_poll.clone();
                        tokio::spawn(async move {
                            let cand = match serde_json::from_str::<RTCIceCandidateInit>(&ice.body) {
                                Ok(c) => c,
                                Err(e) => {
                                    crate::log_warn!(format!("invalid ICE candidate: {}", e));
                                    return;
                                }
                            };
                            let _ = pc.add_ice_candidate(cand).await;
                            crate::log_debug!("ICE candidate processed");
                        });
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                    if *status_poll.read().await == VoiceStatus::Connected {
                        crate::log_info!(format!("voice connected: space={}", space_id_poll));
                        break;
                    }
                }
            });

            *status.write().await = VoiceStatus::Connected;
        });

        Ok(())
    }

    /// 离开语音频道
    pub async fn leave(&self) -> Result<(), String> {
        crate::log_info!(format!("离开语音频道: space_id={}", self.space_id));

        if let Some(mut server) = self.server_handle.write().await.take() {
            server.shutdown();
        }

        {
            let mut peers = self.peers.write().await;
            for (_, peer) in peers.iter() {
                if let Some(ref pc) = peer.pc {
                    let _ = pc.close().await;
                }
            }
            peers.clear();
        }

        *self.status.write().await = VoiceStatus::Disconnected;
        Ok(())
    }

    /// 添加 WebRTC peer
    pub async fn add_peer(&self, peer_id: String, pc: Arc<RTCPeerConnection>) {
        crate::log_info!(format!("add voice peer: space={}, peer={}", self.space_id, peer_id));
        let mut peers = self.peers.write().await;
        peers.insert(peer_id.clone(), WebRtcPeer {
            peer_id,
            pc: Some(pc),
        });
    }

    /// 移除 WebRTC peer
    pub async fn remove_peer(&self, peer_id: &str) {
        crate::log_info!(format!("remove voice peer: space={}, peer={}", self.space_id, peer_id));
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.remove(peer_id) {
            if let Some(pc) = peer.pc {
                let _ = pc.close().await;
            }
        }
    }

    /// 切换麦克风
    pub async fn toggle_mic(&self) -> bool {
        let mut muted = self.mic_muted.write().await;
        *muted = !*muted;
        crate::log_info!(format!("toggle mic: space={}, muted={}", self.space_id, *muted));
        *muted
    }

    /// 切换扬声器
    pub async fn toggle_speaker(&self) -> bool {
        let mut muted = self.speaker_muted.write().await;
        *muted = !*muted;
        crate::log_info!(format!("toggle speaker: space={}, muted={}", self.space_id, *muted));
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