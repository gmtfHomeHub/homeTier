//! Android 屏幕共享实现
//!
//! 使用 MediaProjection API 实现屏幕采集

#[cfg(target_os = "android")]
use crate::screen::mobile::mod::{
    ScreenShareConfig, ScreenSharePlatform, ScreenShareStatus, AndroidScreenSharePlatform,
};

#[cfg(target_os = "android")]
use jni::{
    objects::{GlobalRef, JClass, JObject, JString},
    sys::{jboolean, jint, jlong},
    JNIEnv, JavaVM,
};

#[cfg(target_os = "android")]
use std::sync::{Arc, Mutex};

/// Android 屏幕共享平台实现
#[cfg(target_os = "android")]
pub struct AndroidScreenSharePlatform {
    /// JavaVM 指针
    java_vm: Option<Arc<JavaVM>>,
    /// 全局引用到 Kotlin ScreenShareManager 实例
    screen_share_manager: Option<GlobalRef>,
    /// 配置
    config: Option<ScreenShareConfig>,
    /// 状态
    status: ScreenShareStatus,
    /// 宽度
    width: u32,
    /// 高度
    height: u32,
    /// 码率
    bitrate: u32,
    /// 帧率
    frame_rate: u32,
}

#[cfg(target_os = "android")]
impl AndroidScreenSharePlatform {
    pub fn new() -> Self {
        Self {
            java_vm: None,
            screen_share_manager: None,
            config: None,
            status: ScreenShareStatus::Disconnected,
            width: 720,
            height: 1280,
            bitrate: 4000000,
            frame_rate: 30,
        }
    }

    /// 设置 JavaVM 和 ScreenShareManager 全局引用
    pub fn set_java_vm(&mut self, java_vm: Arc<JavaVM>, screen_share_manager: GlobalRef) {
        self.java_vm = Some(java_vm);
        self.screen_share_manager = Some(screen_share_manager);
    }

    /// 获取 JNIEnv
    fn get_env(&self) -> Option<JNIEnv> {
        self.java_vm.as_ref().and_then(|vm| vm.get_env().ok())
    }

    /// 调用 Kotlin 方法开始屏幕共享
    fn call_start_sharing(&self) -> Result<(), String> {
        let env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let manager = self.screen_share_manager.as_ref().ok_or("ScreenShareManager 未初始化")?;

        let config = self.config.as_ref().ok_or("配置未初始化")?;

        let result = env.call_method(
            manager.as_obj(),
            "startSharing",
            "(IIII)Z",
            &[
                (self.width as jint).into(),
                (self.height as jint).into(),
                (self.bitrate as jint).into(),
                (self.frame_rate as jint).into(),
            ],
        ).map_err(|e| format!("调用 startSharing 失败: {:?}", e))?;

        let success = result.z().map_err(|e| format!("获取返回值失败: {:?}", e))?;
        if !success {
            return Err("startSharing 返回失败".to_string());
        }
        Ok(())
    }

    /// 调用 Kotlin 方法停止屏幕共享
    fn call_stop_sharing(&self) -> Result<(), String> {
        let env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let manager = self.screen_share_manager.as_ref().ok_or("ScreenShareManager 未初始化")?;

        env.call_method(
            manager.as_obj(),
            "stopSharing",
            "()Z",
            &[],
        ).map_err(|e| format!("调用 stopSharing 失败: {:?}", e))?
            .z().map_err(|e| format!("获取返回值失败: {:?}", e))?;

        Ok(())
    }

    /// 调用 Kotlin 方法设置编码参数
    fn call_set_encoding_params(&self) -> Result<(), String> {
        let env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let manager = self.screen_share_manager.as_ref().ok_or("ScreenShareManager 未初始化")?;

        env.call_method(
            manager.as_obj(),
            "setEncodingParams",
            "(III)V",
            &[
                (self.width as jint).into(),
                (self.height as jint).into(),
                (self.bitrate as jint).into(),
            ],
        ).map_err(|e| format!("调用 setEncodingParams 失败: {:?}", e))?;

        Ok(())
    }
}

#[cfg(target_os = "android")]
#[async_trait::async_trait]
impl ScreenSharePlatform for AndroidScreenSharePlatform {
    async fn initialize(&mut self, config: ScreenShareConfig) -> Result<(), String> {
        self.width = config.width;
        self.height = config.height;
        self.bitrate = config.bitrate;
        self.frame_rate = config.frame_rate;
        self.config = Some(config);
        self.status = ScreenShareStatus::Connecting;
        crate::log_info!("AndroidScreenSharePlatform: 初始化完成");
        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        self.call_start_sharing()?;
        self.status = ScreenShareStatus::Connected;
        crate::log_info!("AndroidScreenSharePlatform: 开始屏幕共享");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        self.call_stop_sharing()?;
        self.status = ScreenShareStatus::Disconnected;
        crate::log_info!("AndroidScreenSharePlatform: 停止屏幕共享");
        Ok(())
    }

    async fn set_encoding_params(&mut self, width: u32, height: u32, bitrate: u32, frame_rate: u32) -> Result<(), String> {
        self.width = width;
        self.height = height;
        self.bitrate = bitrate;
        self.frame_rate = frame_rate;
        self.call_set_encoding_params()?;
        crate::log_info!(format!("AndroidScreenSharePlatform: 编码参数更新 {}x{} @ {}kbps {}fps", width, height, bitrate/1000, frame_rate));
        Ok(())
    }

    fn status(&self) -> ScreenShareStatus {
        self.status
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        self.stop().await
    }
}

/// Android 屏幕共享模块的 JNI 入口点
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_hometier_app_screen_ScreenShareManager_nativeInit(
    mut env: JNIEnv,
    _class: JClass,
    java_vm: jlong,
    screen_share_manager: JObject,
) -> jboolean {
    1
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_hometier_app_screen_ScreenShareManager_nativeOnFrameData(
    mut env: JNIEnv,
    _class: JClass,
    data: jni::objects::JByteArray,
    width: jint,
    height: jint,
) -> jboolean {
    // 从 Kotlin 接收屏幕帧数据（MediaProjection 回调）
    // 需要转发到 easytier P2P 网络或本地编码
    1
}