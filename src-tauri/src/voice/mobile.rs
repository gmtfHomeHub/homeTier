//! 移动端语音模块
//!
//! 提供跨平台语音功能的抽象层和实现

pub mod mod;

// 重新导出主要类型
pub use mod::{
    VoiceConfig,
    VoicePlatform,
    VoicePlatformFactory,
    VoiceStatus,
    MobileVoiceManager,
    AndroidVoicePlatform,
    IOSVoicePlatform,
    DesktopVoicePlatform,
};