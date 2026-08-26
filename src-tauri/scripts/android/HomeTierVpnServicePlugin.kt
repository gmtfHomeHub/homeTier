package com.hometier.app

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.media.projection.MediaProjectionManager
import android.net.VpnService
import android.os.Build
import androidx.activity.result.ActivityResult
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import android.webkit.WebView
import com.hometier.app.screen.ScreenShareManager

@InvokeArg
class StartVpnArgs {
    var spaceId: String? = null
    var ipv4Addr: String? = null
    var routes: Array<String> = emptyArray()
    var dns: String? = null
    var disallowedApplications: Array<String> = emptyArray()
    var mtu: Int? = null
}

@InvokeArg
class ScreenShareArgs {
    var width: Int = 720
    var height: Int = 1280
    var bitrate: Int = 4_000_000
    var fps: Int = 30
}

@TauriPlugin
class HomeTierVpnServicePlugin(private val activity: Activity) : Plugin(activity) {

    override fun load(webView: WebView) {
        TauriEventBus.attach(webView)
        ScreenShareManager.init(activity)
    }

    // ==================== VPN ====================

    @Command
    fun prepareVpn(invoke: Invoke) {
        activity.runOnUiThread {
            val it = VpnService.prepare(activity)
            if (it != null) {
                startActivityForResult(invoke, it, "onPrepareVpnResult")
                return@runOnUiThread
            }
            val ret = JSObject()
            ret.put("granted", true)
            invoke.resolve(ret)
        }
    }

    @ActivityCallback
    fun onPrepareVpnResult(invoke: Invoke, result: ActivityResult) {
        val ret = JSObject()
        ret.put("granted", result.resultCode == Activity.RESULT_OK)
        invoke.resolve(ret)
    }

    @Command
    fun startVpn(invoke: Invoke) {
        val args = invoke.parseArgs(StartVpnArgs::class.java)
        activity.runOnUiThread {
            HomeTierVpnService.self?.onRevoke()

            val it = VpnService.prepare(activity)
            val ret = JSObject()
            if (it != null) {
                ret.put("errorMsg", "need_prepare")
            } else {
                val intent = Intent(activity, HomeTierVpnService::class.java)
                intent.putExtra(HomeTierVpnService.SPACE_ID, args.spaceId)
                intent.putExtra(HomeTierVpnService.IPV4_ADDR, args.ipv4Addr)
                intent.putExtra(HomeTierVpnService.ROUTES, args.routes)
                intent.putExtra(HomeTierVpnService.DNS, args.dns)
                intent.putExtra(HomeTierVpnService.DISALLOWED_APPLICATIONS, args.disallowedApplications)
                intent.putExtra(HomeTierVpnService.MTU, args.mtu)
                // 服务内部会 startForeground，需用 startForegroundService 以符合 Android 8+ 约束
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    ContextCompat.startForegroundService(activity, intent)
                } else {
                    activity.startService(intent)
                }
            }
            invoke.resolve(ret)
        }
    }

    @Command
    fun stopVpn(invoke: Invoke) {
        activity.runOnUiThread {
            HomeTierVpnService.self?.onRevoke()
            activity.stopService(Intent(activity, HomeTierVpnService::class.java))
            invoke.resolve(JSObject())
        }
    }

    @Command
    fun getVpnStatus(invoke: Invoke) {
        val ret = JSObject()
        ret.put("running", HomeTierVpnService.self != null)
        ret.put("ipv4Addr", HomeTierVpnService.ipv4Addr)
        ret.put("routes", HomeTierVpnService.routes)
        ret.put("dns", HomeTierVpnService.dns)
        invoke.resolve(ret)
    }

    // ==================== 屏幕共享（MediaProjection） ====================

    /** 弹出 MediaProjection 权限对话框（系统"开始录制屏幕？"） */
    @Command
    fun requestScreenCapture(invoke: Invoke) {
        activity.runOnUiThread {
            val mpm = activity.getSystemService(Activity.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
            startActivityForResult(invoke, mpm.createScreenCaptureIntent(), "onScreenCaptureResult")
        }
    }

    /** MediaProjection 授权结果回调 */
    @ActivityCallback
    fun onScreenCaptureResult(invoke: Invoke, result: ActivityResult) {
        ScreenShareManager.get()?.onPermissionResult(result.resultCode, result.data)
        val ret = JSObject()
        ret.put("granted", result.resultCode == Activity.RESULT_OK)
        invoke.resolve(ret)
    }

    /** 开始屏幕共享（创建 VirtualDisplay 采集屏幕帧） */
    @Command
    fun startScreenShare(invoke: Invoke) {
        val args = invoke.parseArgs(ScreenShareArgs::class.java)
        val ret = JSObject()
        ret.put("started", ScreenShareManager.get()?.startSharing(args.width, args.height, args.bitrate, args.fps) ?: false)
        invoke.resolve(ret)
    }

    /** 停止屏幕共享 */
    @Command
    fun stopScreenShare(invoke: Invoke) {
        val ret = JSObject()
        ret.put("stopped", ScreenShareManager.get()?.stopSharing() ?: false)
        invoke.resolve(ret)
    }

    /** 设置屏幕共享画质（编码参数） */
    @Command
    fun setScreenShareQuality(invoke: Invoke) {
        val args = invoke.parseArgs(ScreenShareArgs::class.java)
        ScreenShareManager.get()?.setEncodingParams(args.width, args.height, args.bitrate)
        invoke.resolve(JSObject())
    }

    // ==================== 运行时权限（相机 / 麦克风） ====================

    /** 请求相机权限（Android 13+ 需要运行时授权） */
    @Command
    fun requestCameraPermission(invoke: Invoke) {
        activity.runOnUiThread {
            if (ContextCompat.checkSelfPermission(activity, Manifest.permission.CAMERA)
                != PackageManager.PERMISSION_GRANTED
            ) {
                ActivityCompat.requestPermissions(activity, arrayOf(Manifest.permission.CAMERA), REQUEST_CAMERA)
            }
            invoke.resolve(JSObject())
        }
    }

    /** 请求麦克风权限（语音功能需要运行时授权） */
    @Command
    fun requestMicPermission(invoke: Invoke) {
        activity.runOnUiThread {
            if (ContextCompat.checkSelfPermission(activity, Manifest.permission.RECORD_AUDIO)
                != PackageManager.PERMISSION_GRANTED
            ) {
                ActivityCompat.requestPermissions(activity, arrayOf(Manifest.permission.RECORD_AUDIO), REQUEST_MIC)
            }
            invoke.resolve(JSObject())
        }
    }

    companion object {
        private const val REQUEST_CAMERA = 2001
        private const val REQUEST_MIC = 2002
    }
}
