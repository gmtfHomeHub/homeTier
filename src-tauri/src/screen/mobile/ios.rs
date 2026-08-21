//! iOS 屏幕共享平台实现 (stub)
//!
//! 实际实现需要使用 ReplayKit + Broadcast Extension

use crate::screen::mobile::mod::{
    ScreenShareConfig, ScreenSharePlatform, ScreenShareStatus,
};

/// iOS 屏幕共享平台实现 (stub)
pub struct IOSScreenSharePlatform {
    config: Option<ScreenShareConfig>,
    status: ScreenShareStatus,
}

impl IOSScreenSharePlatform {
    pub fn new() -> Self {
        Self {
            config: None,
            status: ScreenShareStatus::Disconnected,
        }
    }
}

#[async_trait::async_trait]
impl ScreenSharePlatform for IOSScreenSharePlatform {
    async fn initialize(&mut self, config: ScreenShareConfig) -> Result<(), String> {
        self.config = Some(config);
        self.status = ScreenShareStatus::Connecting;
        crate::log_info!("IOSScreenSharePlatform: 初始化完成");
        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        self.status = ScreenShareStatus::Connected;
        crate::log_info!("IOSScreenSharePlatform: 开始屏幕共享");
        // TODO: 实现 ReplayKit 录制 + Broadcast Extension
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        self.status = ScreenShareStatus::Disconnected;
        crate::log_info!("IOSScreenSharePlatform: 停止屏幕共享");
        Ok(())
    }

    async fn set_encoding_params(&mut self, width: u32, height: u32, bitrate: u32, frame_rate: u32) -> Result<(), String> {
        crate::log_info!(format!("IOSScreenSharePlatform: 编码参数更新 {}x{} @ {}kbps {}fps", width, height, bitrate/1000, frame_rate));
        Ok(())
    }

    fn status(&self) -> ScreenShareStatus {
        self.status
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        self.stop().await
    }
}