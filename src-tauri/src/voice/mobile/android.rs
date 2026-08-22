//! Android 语音平台实现
//!
//! 使用 JNI 调用 Kotlin 端的 AudioRecord/AudioTrack 实现

#[cfg(target_os = "android")]
use crate::voice::mobile::{
    VoiceConfig, VoicePlatform, VoiceStatus,
};

#[cfg(target_os = "android")]
use jni::{
    objects::{GlobalRef, JClass, JObject, JString},
    sys::{jboolean, jint, jlong},
    JNIEnv, JavaVM,
};

#[cfg(target_os = "android")]
use std::sync::{Arc, Mutex};

#[cfg(target_os = "android")]
use std::os::raw::c_void;

/// Android 语音平台实现
#[cfg(target_os = "android")]
pub struct AndroidVoicePlatform {
    /// JavaVM 指针，用于在回调中获取 JNIEnv
    java_vm: Option<Arc<JavaVM>>,
    /// 全局引用到 Kotlin VoiceManager 实例
    voice_manager: Option<GlobalRef>,
    /// 配置
    config: Option<VoiceConfig>,
    /// 状态
    status: VoiceStatus,
    /// 麦克风静音状态
    mic_muted: bool,
    /// 扬声器静音状态
    speaker_muted: bool,
    /// 音频采样率
    sample_rate: u32,
    /// 声道数
    channels: u16,
    /// 帧大小
    frame_size: usize,
}

#[cfg(target_os = "android")]
impl AndroidVoicePlatform {
    pub fn new() -> Self {
        Self {
            java_vm: None,
            voice_manager: None,
            config: None,
            status: VoiceStatus::Disconnected,
            mic_muted: false,
            speaker_muted: false,
            sample_rate: 48000,
            channels: 1,
            frame_size: 960,
        }
    }

    /// 设置 JavaVM 和 VoiceManager 全局引用
    /// 需在应用启动时从 Kotlin 端调用
    pub fn set_java_vm(&mut self, java_vm: Arc<JavaVM>, voice_manager: GlobalRef) {
        self.java_vm = Some(java_vm);
        self.voice_manager = Some(voice_manager);
    }

    /// 获取 JNIEnv（自动附着到当前线程，支持 tokio 异步线程）
    fn get_env(&self) -> Option<jni::AttachGuard<'_>> {
        self.java_vm.as_ref().and_then(|vm| vm.attach_current_thread().ok())
    }

    /// 调用 Kotlin 方法启动音频
    fn call_start_audio(&self) -> Result<(), String> {
        let mut env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let voice_manager = self.voice_manager.as_ref().ok_or("VoiceManager 未初始化")?;

        let _config = self.config.as_ref().ok_or("配置未初始化")?;

        // 调用 Kotlin 的 startAudio 方法
        let result = env.call_method(
            voice_manager.as_obj(),
            "startAudio",
            "(III)Z",
            &[
                (self.sample_rate as jint).into(),
                (self.channels as jint).into(),
                (self.frame_size as jint).into(),
            ],
        ).map_err(|e| format!("调用 startAudio 失败: {:?}", e))?;

        let success = result.z().map_err(|e| format!("获取返回值失败: {:?}", e))?;
        if !success {
            return Err("startAudio 返回失败".to_string());
        }
        Ok(())
    }

    /// 调用 Kotlin 方法停止音频
    fn call_stop_audio(&self) -> Result<(), String> {
        let mut env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let voice_manager = self.voice_manager.as_ref().ok_or("VoiceManager 未初始化")?;

        env.call_method(
            voice_manager.as_obj(),
            "stopAudio",
            "()Z",
            &[],
        ).map_err(|e| format!("调用 stopAudio 失败: {:?}", e))?
            .z().map_err(|e| format!("获取返回值失败: {:?}", e))?;

        Ok(())
    }

    /// 设置麦克风静音
    fn call_set_mic_muted(&self, muted: bool) -> Result<(), String> {
        let mut env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let voice_manager = self.voice_manager.as_ref().ok_or("VoiceManager 未初始化")?;

        env.call_method(
            voice_manager.as_obj(),
            "setMicMuted",
            "(Z)V",
            &[(muted as jboolean).into()],
        ).map_err(|e| format!("调用 setMicMuted 失败: {:?}", e))?;

        Ok(())
    }

    /// 设置扬声器静音
    fn call_set_speaker_muted(&self, muted: bool) -> Result<(), String> {
        let mut env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let voice_manager = self.voice_manager.as_ref().ok_or("VoiceManager 未初始化")?;

        env.call_method(
            voice_manager.as_obj(),
            "setSpeakerMuted",
            "(Z)V",
            &[(muted as jboolean).into()],
        ).map_err(|e| format!("调用 setSpeakerMuted 失败: {:?}", e))?;

        Ok(())
    }

    /// 发送音频数据到 Kotlin 端
    fn call_send_audio(&self, data: &[u8]) -> Result<(), String> {
        let mut env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let voice_manager = self.voice_manager.as_ref().ok_or("VoiceManager 未初始化")?;

        let byte_array = env.byte_array_from_slice(data)
            .map_err(|e| format!("创建字节数组失败: {:?}", e))?;

        env.call_method(
            voice_manager.as_obj(),
            "sendAudio",
            "([B)V",
            &[(&byte_array).into()],
        ).map_err(|e| format!("调用 sendAudio 失败: {:?}", e))?;

        Ok(())
    }
}

#[cfg(target_os = "android")]
#[async_trait::async_trait]
impl VoicePlatform for AndroidVoicePlatform {
    async fn initialize(&mut self, config: VoiceConfig) -> Result<(), String> {
        self.sample_rate = config.sample_rate;
        self.channels = config.channels;
        self.frame_size = config.frame_size;
        self.config = Some(config);
        self.status = VoiceStatus::Connecting;
        crate::log_info!("AndroidVoicePlatform: 初始化完成");
        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        self.call_start_audio()?;
        self.status = VoiceStatus::Connected;
        crate::log_info!("AndroidVoicePlatform: 启动语音");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        self.call_stop_audio()?;
        self.status = VoiceStatus::Disconnected;
        crate::log_info!("AndroidVoicePlatform: 停止语音");
        Ok(())
    }

    async fn set_mic_muted(&mut self, muted: bool) -> Result<(), String> {
        self.call_set_mic_muted(muted)?;
        self.mic_muted = muted;
        crate::log_info!(format!("AndroidVoicePlatform: 麦克风静音 = {}", muted));
        Ok(())
    }

    async fn set_speaker_muted(&mut self, muted: bool) -> Result<(), String> {
        self.call_set_speaker_muted(muted)?;
        self.speaker_muted = muted;
        crate::log_info!(format!("AndroidVoicePlatform: 扬声器静音 = {}", muted));
        Ok(())
    }

    async fn is_mic_muted(&self) -> bool {
        self.mic_muted
    }

    async fn is_speaker_muted(&self) -> bool {
        self.speaker_muted
    }

    async fn send_audio(&mut self, data: &[u8]) -> Result<(), String> {
        self.call_send_audio(data)
    }

    async fn receive_audio(&mut self, data: &[u8]) -> Result<(), String> {
        // 音频播放由 Kotlin 端处理（通过 AudioTrack）
        // 这里仅记录日志
        crate::log_info!(format!("AndroidVoicePlatform: 接收音频数据 {} 字节", data.len()));
        Ok(())
    }

    fn status(&self) -> VoiceStatus {
        self.status
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        self.stop().await
    }
}

/// Android 语音模块的 JNI 入口点
///
/// 这些函数将由 Kotlin 端调用
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_hometier_app_voice_VoiceManager_nativeInit<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    _java_vm: jlong,
    _voice_manager: JObject<'a>,
) -> jboolean {
    // 这里需要通过全局状态来存储 JavaVM 和 VoiceManager 引用
    // 实际实现需要全局单例或通过 Tauri 插件传递
    // 这里仅作为示例框架
    1
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_hometier_app_voice_VoiceManager_nativeOnAudioData<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
    _data: jni::objects::JByteArray<'a>,
) -> jboolean {
    // 从 Kotlin 接收音频数据（录音回调）
    // 需要转发到 easytier P2P 网络
    1
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_hometier_app_voice_VoiceManager_nativeOnPlaybackData<'a>(
    _env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JByteArray<'a> {
    // 请求播放数据（播放回调）
    // 从网络接收队列获取数据并返回给 AudioTrack
    JNIEnv::new().unwrap().byte_array_from_slice(&[]).unwrap()
}