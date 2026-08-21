//! iOS 语音平台实现 (stub)
//!
//! 实际实现需要在 iOS 端使用 AVAudioEngine

use crate::voice::mobile::mod::{
    VoiceConfig, VoicePlatform, VoiceStatus,
};

/// iOS 语音平台实现 (stub)
pub struct IOSVoicePlatform {
    config: Option<VoiceConfig>,
    status: VoiceStatus,
    mic_muted: bool,
    speaker_muted: bool,
}

impl IOSVoicePlatform {
    pub fn new() -> Self {
        Self {
            config: None,
            status: VoiceStatus::Disconnected,
            mic_muted: false,
            speaker_muted: false,
        }
    }
}

#[async_trait::async_trait]
impl VoicePlatform for IOSVoicePlatform {
    async fn initialize(&mut self, config: VoiceConfig) -> Result<(), String> {
        self.config = Some(config);
        self.status = VoiceStatus::Connecting;
        crate::log_info!("IOSVoicePlatform: 初始化完成");
        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        self.status = VoiceStatus::Connected;
        crate::log_info!("IOSVoicePlatform: 启动语音");
        // TODO: 实现 AVAudioEngine 采集和播放
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        self.status = VoiceStatus::Disconnected;
        crate::log_info!("IOSVoicePlatform: 停止语音");
        Ok(())
    }

    async fn set_mic_muted(&mut self, muted: bool) -> Result<(), String> {
        self.mic_muted = muted;
        crate::log_info!(format!("IOSVoicePlatform: 麦克风静音 = {}", muted));
        Ok(())
    }

    async fn set_speaker_muted(&mut self, muted: bool) -> Result<(), String> {
        self.speaker_muted = muted;
        crate::log_info!(format!("IOSVoicePlatform: 扬声器静音 = {}", muted));
        Ok(())
    }

    async fn is_mic_muted(&self) -> bool {
        self.mic_muted
    }

    async fn is_speaker_muted(&self) -> bool {
        self.speaker_muted
    }

    async fn send_audio(&mut self, data: &[u8]) -> Result<(), String> {
        // TODO: 通过 easytier P2P 发送音频数据
        Ok(())
    }

    async fn receive_audio(&mut self, data: &[u8]) -> Result<(), String> {
        // TODO: 播放接收到的音频数据
        Ok(())
    }

    fn status(&self) -> VoiceStatus {
        self.status
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        self.stop().await
    }
}