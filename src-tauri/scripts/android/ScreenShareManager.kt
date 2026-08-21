package com.hometier.app.screen

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Handler
import android.os.Looper
import android.util.Log

/**
 * 屏幕共享管理器（MediaProjection）
 *
 * 职责：
 * 1. 保存 MediaProjection 权限结果（由 HomeTierVpnServicePlugin.onScreenCaptureResult 回调）
 * 2. 创建 VirtualDisplay 采集屏幕帧
 * 3. 通过 JNI 与 Rust 侧桥接（nativeInit / nativeOnPermissionResult / nativeOnFrameData）
 *
 * 生命周期：
 * - init(activity)：应用启动时由插件 load() 调用，注册 JNI 桥
 * - onPermissionResult(resultCode, data)：用户授权后保存 MediaProjection 实例
 * - startSharing(w,h,bitrate,fps)：由 Rust JNI 调用，创建 VirtualDisplay 开始采集
 * - stopSharing()：释放采集资源
 */
class ScreenShareManager(private val activity: Activity) {

    companion object {
        private const val TAG = "ScreenShareManager"

        @Volatile
        private var instance: ScreenShareManager? = null

        init {
            System.loadLibrary("home_tier_lib")
        }

        fun init(activity: Activity) {
            if (instance == null) {
                instance = ScreenShareManager(activity)
                // vm 参数 Kotlin 无法直接取得，传 0，Rust 侧从 JNIEnv 获取真实 JavaVM
                instance?.nativeInit(0L, instance!!)
                Log.d(TAG, "ScreenShareManager 初始化完成，JNI 桥已注册")
            }
        }

        fun get(): ScreenShareManager? = instance
    }

    private val projectionManager =
        activity.getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager

    @Volatile
    private var mediaProjection: MediaProjection? = null

    @Volatile
    private var virtualDisplay: VirtualDisplay? = null

    private val mainHandler = Handler(Looper.getMainLooper())

    // ---- JNI 桥 ----

    private external fun nativeInit(vm: Long, manager: ScreenShareManager)
    external fun nativeOnPermissionResult(granted: Boolean)
    external fun nativeOnFrameData(data: ByteArray, width: Int, height: Int)

    // ---- 权限结果（由插件回调） ----

    /** 处理 MediaProjection 授权结果 */
    fun onPermissionResult(resultCode: Int, data: Intent?) {
        if (resultCode == Activity.RESULT_OK && data != null) {
            mediaProjection = projectionManager.getMediaProjection(resultCode, data)
            nativeOnPermissionResult(true)
            Log.d(TAG, "MediaProjection 权限已授予")
        } else {
            nativeOnPermissionResult(false)
            Log.d(TAG, "MediaProjection 权限被拒绝")
        }
    }

    // ---- 采集生命周期（由 Rust 侧 JNI 调用） ----

    /** 开始屏幕共享：创建 VirtualDisplay */
    fun startSharing(width: Int, height: Int, bitrate: Int, fps: Int): Boolean {
        val projection = mediaProjection ?: return false
        if (virtualDisplay != null) return true

        virtualDisplay = projection.createVirtualDisplay(
            "HomeTierScreenShare",
            width,
            height,
            activity.resources.displayMetrics.densityDpi,
            DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
            null, // Surface：帧回调由原生层注册（后续接入 ImageReader + H.264 编码）
            null,
            null
        )
        Log.d(TAG, "VirtualDisplay 已创建 ${width}x${height} @${bitrate}bps ${fps}fps")
        return true
    }

    /** 停止屏幕共享 */
    fun stopSharing(): Boolean {
        virtualDisplay?.release()
        virtualDisplay = null
        mediaProjection?.stop()
        mediaProjection = null
        Log.d(TAG, "屏幕共享已停止")
        return true
    }

    /** 更新编码参数（透传给原生编码器，后续实现） */
    fun setEncodingParams(width: Int, height: Int, bitrate: Int) {
        Log.d(TAG, "编码参数更新 ${width}x${height} @${bitrate}bps")
        // TODO: 重建 ImageReader / 编码器（VideoEncoder）
    }
}
