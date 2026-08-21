//! 移动端屏幕共享模块 - 跨平台屏幕共享抽象层
//!
//! 定义跨平台屏幕共享接口，Android 使用 MediaProjection，
//! iOS 使用 ReplayKit，桌面端使用 WebRTC

use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "ios")]
pub mod ios;

/// 屏幕共享状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenShareStatus {
    Disconnected,
    Connecting,
    Connected,
    Paused,
}

impl ScreenShareStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScreenShareStatus::Disconnected => "disconnected",
            ScreenShareStatus::Connecting => "connecting",
            ScreenShareStatus::Connected => "connected",
            ScreenShareStatus::Paused => "paused",
        }
    }
}

/// 屏幕共享配置
#[derive(Debug, Clone)]
pub struct ScreenShareConfig {
    pub width: u32,
    pub height: u32,
    pub bitrate: u32,
    pub frame_rate: u32,
    pub quality: ScreenQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenQuality {
    Low,      // 480p, 1Mbps
    Medium,   // 720p, 4Mbps
    High,     // 1080p, 8Mbps
    Ultra,    // 1440p, 15Mbps
}

impl Default for ScreenShareConfig {
    fn default() -> Self {
        Self {
            width: 720,
            height: 1280,
            bitrate: 4_000_000, // 4Mbps
            frame_rate: 30,
            quality: ScreenQuality::Medium,
        }
    }
}

impl ScreenQuality {
    pub fn to_config(self) -> ScreenShareConfig {
        match self {
            ScreenQuality::Low => ScreenShareConfig {
                width: 480,
                height: 854,
                bitrate: 1_000_000,
                frame_rate: 20,
                quality: ScreenQuality::Low,
            },
            ScreenQuality::Medium => ScreenShareConfig {
                width: 720,
                height: 1280,
                bitrate: 4_000_000,
                frame_rate: 30,
                quality: ScreenQuality::Medium,
            },
            ScreenQuality::High => ScreenShareConfig {
                width: 1080,
                height: 1920,
                bitrate: 8_000_000,
                frame_rate: 30,
                quality: ScreenQuality::High,
            },
            ScreenQuality::Ultra => ScreenShareConfig {
                width: 1440,
                height: 2560,
                bitrate: 15_000_000,
                frame_rate: 30,
                quality: ScreenQuality::Ultra,
            },
        }
    }
}

/// 跨平台屏幕共享平台接口
#[async_trait::async_trait]
pub trait ScreenSharePlatform: Send + Sync {
    /// 初始化屏幕共享
    async fn initialize(&mut self, config: ScreenShareConfig) -> Result<(), String>;

    /// 开始屏幕共享
    async fn start(&mut self) -> Result<(), String>;

    /// 停止屏幕共享
    async fn stop(&mut self) -> Result<(), String>;

    /// 设置编码参数
    async fn set_encoding_params(&mut self, width: u32, height: u32, bitrate: u32, frame_rate: u32) -> Result<(), String>;

    /// 获取当前状态
    fn status(&self) -> ScreenShareStatus;

    /// 请求屏幕共享权限（Android MediaProjection 对话框 / iOS ReplayKit 引导）
    /// 默认实现：不触发任何系统交互，返回 Ok
    async fn request_permission(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// 打开系统设置（iOS ReplayKit 需手动开启屏幕录制）
    /// 默认实现：不打开任何设置页，返回 Ok
    async fn open_settings(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// 请求相机权限（视频通话使用）
    /// 默认实现：不触发运行时权限，返回 Ok
    async fn request_camera_permission(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// 清理资源
    async fn shutdown(&mut self) -> Result<(), String>;
}

/// 屏幕共享平台工厂
pub struct ScreenSharePlatformFactory;

impl ScreenSharePlatformFactory {
    /// 创建平台特定的屏幕共享实现
    pub fn create() -> Box<dyn ScreenSharePlatform> {
        #[cfg(target_os = "android")]
        {
            Box::new(android::AndroidScreenSharePlatform::new())
        }
        #[cfg(target_os = "ios")]
        {
            Box::new(ios::IOSScreenSharePlatform::new())
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            // 桌面端使用现有的 WebRTC 实现
            Box::new(DesktopScreenSharePlatform::new())
        }
    }
}

/// iOS 屏幕共享平台实现 (stub)
#[cfg(target_os = "ios")]
pub struct IOSScreenSharePlatform {
    config: Option<ScreenShareConfig>,
    status: ScreenShareStatus,
}

#[cfg(target_os = "ios")]
impl IOSScreenSharePlatform {
    pub fn new() -> Self {
        Self {
            config: None,
            status: ScreenShareStatus::Disconnected,
        }
    }
}

#[cfg(target_os = "ios")]
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
        // TODO: 实现 ReplayKit 录制
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

/// 桌面端屏幕共享平台实现 (使用现有 WebRTC)
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct DesktopScreenSharePlatform;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl DesktopScreenSharePlatform {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[async_trait::async_trait]
impl ScreenSharePlatform for DesktopScreenSharePlatform {
    async fn initialize(&mut self, _config: ScreenShareConfig) -> Result<(), String> {
        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        // 桌面端使用现有 WebRTC ScreenShareEngine
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }

    async fn set_encoding_params(&mut self, _width: u32, _height: u32, _bitrate: u32, _frame_rate: u32) -> Result<(), String> {
        Ok(())
    }

    fn status(&self) -> ScreenShareStatus {
        ScreenShareStatus::Disconnected
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// 移动端屏幕共享管理器
pub struct MobileScreenShareManager {
    platform: Box<dyn ScreenSharePlatform>,
    config: ScreenShareConfig,
}

impl MobileScreenShareManager {
    pub fn new(config: ScreenShareConfig) -> Self {
        Self {
            platform: ScreenSharePlatformFactory::create(),
            config,
        }
    }

    pub async fn initialize(&mut self) -> Result<(), String> {
        self.platform.initialize(self.config.clone()).await
    }

    pub async fn start_sharing(&mut self) -> Result<(), String> {
        self.platform.start().await
    }

    pub async fn stop_sharing(&mut self) -> Result<(), String> {
        self.platform.stop().await
    }

    pub async fn request_permission(&mut self) -> Result<(), String> {
        self.platform.request_permission().await
    }

    pub async fn open_settings(&mut self) -> Result<(), String> {
        self.platform.open_settings().await
    }

    pub async fn request_camera_permission(&mut self) -> Result<(), String> {
        self.platform.request_camera_permission().await
    }

    pub async fn set_quality(&mut self, quality: ScreenQuality) -> Result<(), String> {
        self.config = quality.to_config();
        self.platform.set_encoding_params(
            self.config.width,
            self.config.height,
            self.config.bitrate,
            self.config.frame_rate,
        ).await
    }

    pub fn status(&self) -> ScreenShareStatus {
        self.platform.status()
    }
}