//! 跨平台互通优化 - 统一编解码与信令
//!
//! 统一桌面端 WebRTC 与移动端原生音频的编解码格式、信令协议
//! 实现跨平台语音/屏幕共享互通

use crate::voice::opus::{OpusEncoder, OpusDecoder, VoiceConfig, AudioPacket, AudioQueue};
use crate::voice::mobile::mod::{VoicePlatform, VoiceConfig, VoiceStatus};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use std::time::{Duration, Instant};

/// 统一编解码配置
#[derive(Debug, Clone)]
pub struct UnifiedCodecConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_size: usize,
    pub bitrate: u32,
    pub complexity: u8,
    pub use_dtx: bool,
    pub use_fec: bool,
}

impl Default for UnifiedCodecConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 1,
            frame_size: 960, // 20ms at 48kHz
            bitrate: 64000,
            complexity: 10,
            use_dtx: true,
            use_fec: true,
        }
    }
}

/// 统一编解码器
pub struct UnifiedCodec {
    encoder: Option<OpusEncoder>,
    decoder: Option<OpusDecoder>,
    config: UnifiedCodecConfig,
}

impl UnifiedCodec {
    pub fn new(config: UnifiedCodecConfig) -> Result<Self, String> {
        let encoder = OpusEncoder::new(config.sample_rate, config.channels, config.frame_size)?;
        let decoder = OpusDecoder::new(config.sample_rate, config.channels, config.frame_size)?;
        
        Ok(Self {
            encoder: Some(encoder),
            decoder: Some(decoder),
            config,
        })
    }

    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>, String> {
        self.encoder.as_mut().ok_or("编码器未初始化")?.encode(pcm)
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<i16>, String> {
        self.decoder.as_mut().ok_or("解码器未初始化")?.decode(data)
    }
}

/// 自适应码率控制器
pub struct AdaptiveBitrateController {
    current_bitrate: u32,
    min_bitrate: u32,
    max_bitrate: u32,
    target_packet_loss: f32,
    last_adjustment: Instant,
    adjustment_interval: Duration,
    rtt_history: Vec<u32>,
    packet_loss_history: Vec<f32>,
}

impl AdaptiveBitrateController {
    pub fn new(initial_bitrate: u32, min_bitrate: u32, max_bitrate: u32) -> Self {
        Self {
            current_bitrate: initial_bitrate,
            min_bitrate,
            max_bitrate,
            target_packet_loss: 0.01, // 目标丢包率 1%
            last_adjustment: Instant::now(),
            adjustment_interval: Duration::from_secs(2),
            rtt_history: Vec::with_capacity(10),
            packet_loss_history: Vec::with_capacity(10),
        }
    }

    /// 记录网络统计
    pub fn record_stats(&mut self, rtt_ms: u32, packet_loss: f32) {
        self.rtt_history.push(rtt_ms);
        self.packet_loss_history.push(packet_loss);
        
        if self.rtt_history.len() > 10 {
            self.rtt_history.remove(0);
        }
        if self.packet_loss_history.len() > 10 {
            self.packet_loss_history.remove(0);
        }
    }

    /// 计算自适应码率
    pub fn calculate_bitrate(&mut self) -> u32 {
        let now = Instant::now();
        if now.duration_since(self.last_adjustment) < self.adjustment_interval {
            return self.current_bitrate;
        }

        let avg_rtt = if self.rtt_history.is_empty() { 
            50 
        } else { 
            self.rtt_history.iter().sum::<u32>() / self.rtt_history.len() as u32 
        };
        
        let avg_loss = if self.packet_loss_history.is_empty() { 
            0.0 
        } else { 
            self.packet_loss_history.iter().sum::<f32>() / self.packet_loss_history.len() as f32 
        };

        let mut new_bitrate = self.current_bitrate;

        // 根据丢包率调整
        if avg_loss > self.target_packet_loss * 2.0 {
            // 丢包率过高，降低码率
            new_bitrate = (self.current_bitrate as f32 * 0.85) as u32;
        } else if avg_loss < self.target_packet_loss * 0.5 && avg_rtt < 100 {
            // 网络良好，尝试提高码率
            new_bitrate = (self.current_bitrate as f32 * 1.1) as u32;
        }

        // RTT 过高也降低码率
        if avg_rtt > 200 {
            new_bitrate = (new_bitrate as f32 * 0.9) as u32;
        }

        // 限制在范围内
        new_bitrate = new_bitrate.clamp(32000, 128000);

        if new_bitrate != self.current_bitrate {
            crate::log_info!(format!("自适应码率调整: {} -> {} kbps (丢包: {:.2}%, RTT: {}ms)", 
                self.current_bitrate / 1000, new_bitrate / 1000, avg_loss * 100.0, avg_rtt));
            self.current_bitrate = new_bitrate;
            self.last_adjustment = now;
        }

        self.current_bitrate
    }

    pub fn get_bitrate(&self) -> u32 {
        self.current_bitrate
    }
}

/// 统一信令消息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum UnifiedSignalMessage {
    /// 音频流控制
    AudioControl {
        action: AudioAction,
        space_id: String,
        peer_id: String,
    },
    /// 视频/屏幕流控制
    VideoControl {
        action: VideoAction,
        space_id: String,
        peer_id: String,
        config: Option<VideoConfig>,
    },
    /// ICE 候选
    IceCandidate {
        space_id: String,
        peer_id: String,
        candidate: IceCandidateData,
    },
    /// SDP 交换
    SdpOffer {
        space_id: String,
        peer_id: String,
        sdp: String,
    },
    SdpAnswer {
        space_id: String,
        peer_id: String,
        sdp: String,
    },
    /// 网络质量反馈
    NetworkFeedback {
        space_id: String,
        peer_id: String,
        rtt_ms: u32,
        packet_loss: f32,
        jitter_ms: u32,
        bitrate_kbps: u32,
    },
    /// 质量切换
    QualityChange {
        space_id: String,
        peer_id: String,
        quality: String,
    },
    /// 心跳/保活
    Ping {
        space_id: String,
        peer_id: String,
        timestamp: u64,
    },
    Pong {
        space_id: String,
        peer_id: String,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AudioAction {
    Start,
    Stop,
    MuteMic,
    UnmuteMic,
    MuteSpeaker,
    UnmuteSpeaker,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum VideoAction {
    Start,
    Stop,
    Pause,
    Resume,
    ChangeQuality,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VideoConfig {
    width: u32,
    height: u32,
    bitrate: u32,
    frame_rate: u32,
    codec: String, // "vp8", "vp9", "h264"
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IceCandidateData {
    candidate: String,
    sdp_mid: String,
    sdp_mline_index: u32,
}

/// 统一信令处理器
pub struct UnifiedSignalingHandler {
    space_id: String,
    local_peer_id: String,
    bitrate_controller: AdaptiveBitrateController,
    pending_ice_candidates: Arc<Mutex<Vec<IceCandidateData>>>,
}

impl UnifiedSignalingHandler {
    pub fn new(space_id: String, local_peer_id: String) -> Self {
        Self {
            space_id,
            local_peer_id,
            bitrate_controller: AdaptiveBitrateController::new(64000, 32000, 128000),
            pending_ice_candidates: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 处理接收到的信令消息
    pub async fn handle_message(&mut self, message: UnifiedSignalMessage, from_peer: &str) -> Result<Option<UnifiedSignalMessage>, String> {
        match message {
            UnifiedSignalMessage::AudioControl { action, space_id, peer_id } => {
                if space_id != self.space_id {
                    return Err("空间 ID 不匹配".to_string());
                }
                self.handle_audio_control(action, peer_id).await
            }
            UnifiedSignalMessage::VideoControl { action, space_id, peer_id, config } => {
                if space_id != self.space_id {
                    return Err("空间 ID 不匹配".to_string());
                }
                self.handle_video_control(action, peer_id, config).await
            }
            UnifiedSignalMessage::IceCandidate { space_id, peer_id, candidate } => {
                if space_id != self.space_id {
                    return Err("空间 ID 不匹配".to_string());
                }
                self.handle_ice_candidate(candidate, peer_id).await
            }
            UnifiedSignalMessage::SdpOffer { space_id, peer_id, sdp } => {
                if space_id != self.space_id {
                    return Err("空间 ID 不匹配".to_string());
                }
                self.handle_sdp_offer(sdp, peer_id).await
            }
            UnifiedSignalMessage::SdpAnswer { space_id, peer_id, sdp } => {
                if space_id != self.space_id {
                    return Err("空间 ID 不匹配".to_string());
                }
                self.handle_sdp_answer(sdp, peer_id).await
            }
            UnifiedSignalMessage::NetworkFeedback { space_id, peer_id, rtt_ms, packet_loss, jitter_ms, bitrate_kbps } => {
                if space_id != self.space_id {
                    return Err("空间 ID 不匹配".to_string());
                }
                self.handle_network_feedback(rtt_ms, packet_loss, jitter_ms, bitrate_kbps).await
            }
            UnifiedSignalMessage::QualityChange { space_id, peer_id, quality } => {
                if space_id != self.space_id {
                    return Err("空间 ID 不匹配".to_string());
                }
                self.handle_quality_change(quality).await
            }
            UnifiedSignalMessage::Ping { space_id, peer_id, timestamp } => {
                if space_id != self.space_id {
                    return Err("空间 ID 不匹配".to_string());
                }
                Ok(Some(UnifiedSignalMessage::Pong {
                    space_id: self.space_id.clone(),
                    peer_id: self.local_peer_id.clone(),
                    timestamp,
                }))
            }
            UnifiedSignalMessage::Pong { .. } => {
                // 处理 Pong，计算 RTT
                Ok(None)
            }
            UnifiedSignalMessage::QualityChange { .. } => {
                Ok(None)
            }
        }
    }

    async fn handle_audio_control(&mut self, action: AudioAction, peer_id: String) -> Result<Option<UnifiedSignalMessage>, String> {
        // TODO: 实现音频控制逻辑
        crate::log_info!(format!("音频控制: {:?} from {}", action, peer_id));
        Ok(None)
    }

    async fn handle_video_control(&mut self, action: VideoAction, peer_id: String, config: Option<VideoConfig>) -> Result<Option<UnifiedSignalMessage>, String> {
        // TODO: 实现视频控制逻辑
        crate::log_info!(format!("视频控制: {:?} from {}", action, peer_id));
        Ok(None)
    }

    async fn handle_ice_candidate(&mut self, candidate: IceCandidateData, peer_id: String) -> Result<Option<UnifiedSignalMessage>, String> {
        let mut pending = self.pending_ice_candidates.lock().await;
        pending.push(candidate);
        Ok(None)
    }

    async fn handle_sdp_offer(&mut self, sdp: String, peer_id: String) -> Result<Option<UnifiedSignalMessage>, String> {
        // TODO: 处理 SDP Offer
        crate::log_info!(format!("收到 SDP Offer from {}", peer_id));
        Ok(None)
    }

    async fn handle_sdp_answer(&mut self, sdp: String, peer_id: String) -> Result<Option<UnifiedSignalMessage>, String> {
        // TODO: 处理 SDP Answer
        crate::log_info!(format!("收到 SDP Answer from {}", peer_id));
        Ok(None)
    }

    async fn handle_network_feedback(&mut self, rtt_ms: u32, packet_loss: f32, jitter_ms: u32, bitrate_kbps: u32) -> Result<Option<UnifiedSignalMessage>, String> {
        self.bitrate_controller.record_stats(rtt_ms, packet_loss);
        let new_bitrate = self.bitrate_controller.calculate_bitrate();
        
        if self.current_bitrate != new_bitrate {
            crate::log_info!(format!("码率自适应调整: {} kbps", new_bitrate / 1000));
        }
        
        Ok(None)
    }

    async fn handle_quality_change(&mut self, quality: String) -> Result<Option<UnifiedSignalMessage>, String> {
        crate::log_info!(format!("质量切换请求: {}", quality));
        Ok(None)
    }
}

/// 跨平台媒体会话管理器
pub struct CrossPlatformMediaSession {
    space_id: String,
    local_peer_id: String,
    codec: UnifiedCodec,
    signaling: UnifiedSignalingHandler,
    bitrate_controller: AdaptiveBitrateController,
    audio_queue: Arc<Mutex<VecDeque<AudioPacket>>>,
    video_queue: Arc<Mutex<VecDeque<AudioPacket>>>,
    stats: MediaSessionStats,
}

use std::collections::VecDeque;

#[derive(Debug, Default, Clone)]
pub struct MediaSessionStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_lost: u32,
    pub rtt_ms: u32,
    pub jitter_ms: u32,
    pub current_bitrate_kbps: u32,
    pub packet_loss_rate: f32,
}

impl CrossPlatformMediaSession {
    pub fn new(space_id: String, local_peer_id: String) -> Result<Self, String> {
        let config = UnifiedCodecConfig::default();
        let codec = UnifiedCodec::new(config.clone())?;
        
        Ok(Self {
            space_id,
            local_peer_id,
            codec,
            signaling: UnifiedSignalingHandler::new(space_id, local_peer_id),
            bitrate_controller: AdaptiveBitrateController::new(64000, 32000, 128000),
            audio_queue: Arc::new(Mutex::new(VecDeque::new())),
            video_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats: MediaSessionStats::default(),
        })
    }

    /// 处理接收到的音频数据
    pub async fn process_received_audio(&mut self, packet: AudioPacket) -> Result<Vec<i16>, String> {
        let mut codec = self.codec;
        let pcm = codec.decode(&packet.data)?;
        
        self.stats.bytes_received += packet.data.len() as u64;
        self.stats.packets_received += 1;
        
        Ok(pcm)
    }

    /// 准备发送音频数据
    pub async fn prepare_send_audio(&mut self, pcm: &[i16]) -> Result<AudioPacket, String> {
        let mut codec = self.codec;
        let data = codec.encode(pcm)?;
        
        let packet = AudioPacket::new(
            data,
            self.codec.config.sample_rate,
            self.codec.config.channels,
            self.codec.config.frame_size,
        );
        
        self.stats.bytes_sent += packet.data.len() as u64;
        self.stats.packets_sent += 1;
        
        Ok(packet)
    }

    /// 获取会话统计
    pub fn get_stats(&self) -> MediaSessionStats {
        self.stats.clone()
    }

    /// 处理信令消息
    pub async fn handle_signal(&mut self, message: UnifiedSignalMessage, from: &str) -> Result<Option<UnifiedSignalMessage>, String> {
        self.signaling.handle_message(message, from).await
    }
}