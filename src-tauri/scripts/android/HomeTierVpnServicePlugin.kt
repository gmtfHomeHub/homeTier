package com.hometier.app

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import android.webkit.WebView

@InvokeArg
class StartVpnArgs {
    var spaceId: String? = null
    var ipv4Addr: String? = null
    var routes: Array<String> = emptyArray()
    var dns: String? = null
    var disallowedApplications: Array<String> = emptyArray()
    var mtu: Int? = null
}

@TauriPlugin
class HomeTierVpnServicePlugin(private val activity: Activity) : Plugin(activity) {

    override fun load(webView: WebView) {
        TauriEventBus.attach(webView)
    }

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
                activity.startService(intent)
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
}