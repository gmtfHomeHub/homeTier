//! 移动端屏幕共享模块
//!
//! 提供跨平台屏幕共享功能的抽象层和实现

pub mod mod;

// 重新导出主要类型
pub use mod::{
    ScreenShareConfig,
    ScreenSharePlatform,
    ScreenSharePlatformFactory,
    ScreenShareStatus,
    ScreenQuality,
    MobileScreenShareManager,
    AndroidScreenSharePlatform,
    IOSScreenSharePlatform,
    DesktopScreenSharePlatform,
};