//! Android 屏幕共享实现
//!
//! 使用 MediaProjection API 实现屏幕采集

#[cfg(target_os = "android")]
use crate::screen::mobile::{
    ScreenShareConfig, ScreenSharePlatform, ScreenShareStatus,
};

#[cfg(target_os = "android")]
use jni::{
    objects::{GlobalRef, JObject},
    sys::{jboolean, jint, jlong},
    JNIEnv, JavaVM,
};

#[cfg(target_os = "android")]
use std::sync::{Arc, OnceLock};

/// JNI 桥接：由 Kotlin `ScreenShareManager.nativeInit` 在应用启动时调用，
/// 将 JavaVM 与 ScreenShareManager 实例全局引用交给 Rust 侧，
/// 使后续所有 JNI 调用（start/stop/quality）都能跨线程工作。
#[cfg(target_os = "android")]
struct AndroidJniBridge {
    java_vm: Arc<JavaVM>,
    screen_share_manager: GlobalRef,
}

#[cfg(target_os = "android")]
static JNI_BRIDGE: OnceLock<AndroidJniBridge> = OnceLock::new();

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
        let mut platform = Self {
            java_vm: None,
            screen_share_manager: None,
            config: None,
            status: ScreenShareStatus::Disconnected,
            width: 720,
            height: 1280,
            bitrate: 4000000,
            frame_rate: 30,
        };
        // 应用启动时 nativeInit 已注册 JNI 桥，新实例自动接管
        if let Some(bridge) = JNI_BRIDGE.get() {
            platform.set_java_vm(bridge.java_vm.clone(), bridge.screen_share_manager.clone());
        }
        platform
    }

    /// 设置 JavaVM 和 ScreenShareManager 全局引用
    pub fn set_java_vm(&mut self, java_vm: Arc<JavaVM>, screen_share_manager: GlobalRef) {
        self.java_vm = Some(java_vm);
        self.screen_share_manager = Some(screen_share_manager);
    }

    /// 获取 JNIEnv（自动附着到当前线程，支持 tokio 异步线程）
    fn get_env(&self) -> Option<jni::AttachGuard<'_>> {
        self.java_vm.as_ref().and_then(|vm| vm.attach_current_thread().ok())
    }

    /// 调用 Kotlin 方法开始屏幕共享
    fn call_start_sharing(&self) -> Result<(), String> {
        let mut env = self.get_env().ok_or("无法获取 JNIEnv")?;
        let manager = self.screen_share_manager.as_ref().ok_or("ScreenShareManager 未初始化")?;

        let _config = self.config.as_ref().ok_or("配置未初始化")?;

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
        let mut env = self.get_env().ok_or("无法获取 JNIEnv")?;
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
        let mut env = self.get_env().ok_or("无法获取 JNIEnv")?;
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

    async fn request_permission(&mut self) -> Result<(), String> {
        // MediaProjection 权限对话框由 Android 原生插件
        // (HomeTierVpnServicePlugin.requestScreenCapture) 触发，
        // 这里返回 Ok，避免重复弹窗。
        crate::log_info!("AndroidScreenSharePlatform: MediaProjection 权限由原生插件触发");
        Ok(())
    }

    async fn open_settings(&mut self) -> Result<(), String> {
        crate::log_info!("AndroidScreenSharePlatform: 无需打开系统设置");
        Ok(())
    }

    async fn request_camera_permission(&mut self) -> Result<(), String> {
        // 相机运行时权限由 Android 原生插件 (requestCameraPermission) 触发
        crate::log_info!("AndroidScreenSharePlatform: 相机权限由原生插件触发");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        self.stop().await
    }
}

/// Android 屏幕共享模块的 JNI 入口点
///
/// Kotlin 侧 ScreenShareManager.nativeInit(vm, this) 调用，注册 JNI 桥。
/// vm 参数由 Kotlin 传 0（Kotlin 无法直接取得 JavaVM 指针），
/// Rust 侧通过 JNIEnv.get_java_vm() 获取真实 JavaVM。
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_hometier_app_screen_ScreenShareManager_nativeInit(
    env: JNIEnv,
    _this: JObject,
    _java_vm: jlong,
    screen_share_manager: JObject,
) -> jboolean {
    let vm = match env.get_java_vm() {
        Ok(vm) => Arc::new(vm),
        Err(e) => {
            crate::log_error!(format!("ScreenShareManager nativeInit 获取 JavaVM 失败: {:?}", e));
            return 0;
        }
    };
    let global = match env.new_global_ref(screen_share_manager) {
        Ok(g) => g,
        Err(e) => {
            crate::log_error!(format!("ScreenShareManager nativeInit 创建全局引用失败: {:?}", e));
            return 0;
        }
    };
    match JNI_BRIDGE.set(AndroidJniBridge { java_vm: vm, screen_share_manager: global }) {
        Ok(()) => {
            crate::log_info!("ScreenShareManager JNI 桥接已初始化");
            1
        }
        Err(_) => {
            crate::log_warn!("ScreenShareManager JNI 桥接重复初始化");
            1
        }
    }
}

/// Kotlin ScreenShareManager.nativeOnPermissionResult(granted) 回调
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_hometier_app_screen_ScreenShareManager_nativeOnPermissionResult(
    env: JNIEnv,
    _this: JObject,
    granted: jboolean,
) {
    crate::log_info!(format!("MediaProjection 权限结果: granted={}", granted != 0));
}

/// Kotlin ScreenShareManager.nativeOnFrameData(data, width, height) 回调
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_hometier_app_screen_ScreenShareManager_nativeOnFrameData<'a>(
    env: JNIEnv<'a>,
    _this: JObject<'a>,
    _data: jni::objects::JByteArray<'a>,
    _width: jint,
    _height: jint,
) -> jboolean {
    // 从 Kotlin 接收屏幕帧数据（MediaProjection 回调）
    // 需要转发到 easytier P2P 网络或本地编码（后续实现）
    crate::log_debug!("ScreenShareManager: 收到屏幕帧数据");
    1
}