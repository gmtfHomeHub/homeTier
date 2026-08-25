package com.hometier.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.os.Bundle
import android.webkit.WebView
import android.util.Log

/**
 * HomeTier VpnService
 *
 * Creates a TUN interface and passes the file descriptor back to the app
 * via TauriEventBus using evaluateJavascript. Based on EasyTier's TauriVpnService.
 *
 * The fd is an int and both Kotlin and Rust run in the same process on
 * Android, so the numeric fd is valid on the Rust side.
 */
class HomeTierVpnService : VpnService() {
    companion object {
        @JvmField var self: HomeTierVpnService? = null
        @JvmField var ipv4Addr: String? = null
        @JvmField var routes: Array<String> = emptyArray()
        @JvmField var dns: String? = null

        const val IPV4_ADDR = "IPV4_ADDR"
        const val ROUTES = "ROUTES"
        const val DNS = "DNS"
        const val DISALLOWED_APPLICATIONS = "DISALLOWED_APPLICATIONS"
        const val MTU = "MTU"
        const val SPACE_ID = "SPACE_ID"
        const val CHANNEL_ID = "homeTierVpnChannel"
        const val NOTIFICATION_ID = 1001
    }

    private lateinit var vpnInterface: ParcelFileDescriptor
    private var currentSpaceId: String = ""

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d("HomeTierVpn", "onStartCommand: ${intent?.extras}")
        val args = intent?.extras
        currentSpaceId = args?.getString(SPACE_ID) ?: ""
        ipv4Addr = args?.getString(IPV4_ADDR)
        routes = args?.getStringArray(ROUTES) ?: emptyArray()
        dns = args?.getString(DNS)

        // Create notification channel and start foreground service (Android 14+ requirement)
        createNotificationChannel()
        val notification = buildNotification("准备连接…")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }

        vpnInterface = createVpnInterface(args)

        // 发送 fd 到前端
        val eventData = org.json.JSONObject().apply {
            put("spaceId", currentSpaceId)
            put("fd", vpnInterface.fd)
        }
        TauriEventBus.emit("vpn:tun-ready", eventData.toString())

        updateNotification("已连接")

        return START_STICKY
    }

    override fun onCreate() {
        super.onCreate()
        self = this
    }

    override fun onDestroy() {
        super.onDestroy()
        disconnect()
        self = null
    }

    override fun onRevoke() {
        super.onRevoke()
        disconnect()
        self = null
    }

    private fun disconnect() {
        if (self == this && this::vpnInterface.isInitialized) {
            val eventData = org.json.JSONObject().apply {
                put("spaceId", currentSpaceId)
                put("status", "stopped")
            }
            TauriEventBus.emit("vpn:status-changed", eventData.toString())
            vpnInterface.close()
        }
        clearStatus()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            stopForeground(STOP_FOREGROUND_REMOVE)
        } else {
            @Suppress("DEPRECATION") stopForeground(true)
        }
        stopSelf()
    }

    private fun clearStatus() {
        ipv4Addr = null
        routes = emptyArray()
        dns = null
    }

    private fun createVpnInterface(args: Bundle?): ParcelFileDescriptor {
        val builder = Builder()
            .setSession("HomeTierVpn")
            .setBlocking(false)

        val mtu = args?.getInt(MTU) ?: 1500
        val ipv4Addr = args?.getString(IPV4_ADDR) ?: "10.144.144.1/24"
        val dns = args?.getString(DNS)
        val routes = args?.getStringArray(ROUTES) ?: emptyArray()
        val disallowedApplications = args?.getStringArray(DISALLOWED_APPLICATIONS) ?: emptyArray()

        val ipParts = ipv4Addr.split("/")
        if (ipParts.size != 2) throw IllegalArgumentException("Invalid IP addr string")
        builder.addAddress(ipParts[0], ipParts[1].toInt())
        builder.addAddress("fd00::1", 128)

        builder.setMtu(mtu)
        dns?.let { builder.addDnsServer(it) }

        for (route in routes) {
            val routeParts = route.split("/")
            if (routeParts.size != 2) throw IllegalArgumentException("Invalid route cidr string")
            builder.addRoute(routeParts[0], routeParts[1].toInt())
        }

        for (app in disallowedApplications) {
            builder.addDisallowedApplication(app)
        }

        // Exclude self to prevent routing loop
        builder.addDisallowedApplication(packageName)

        return builder.also {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                it.setMetered(false)
            }
        }.establish() ?: throw IllegalStateException("Failed to init VpnService")
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val nm = getSystemService(NotificationManager::class.java)
            if (nm.getNotificationChannel(CHANNEL_ID) == null) {
                val channel = NotificationChannel(
                    CHANNEL_ID, "homeTier VPN",
                    NotificationManager.IMPORTANCE_LOW
                ).apply {
                    description = "homeTier VPN 连接状态"
                    setShowBadge(false)
                }
                nm.createNotificationChannel(channel)
            }
        }
    }

    private fun buildNotification(text: String): Notification {
        val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
        val pi = launchIntent?.let {
            PendingIntent.getActivity(
                this, 0, it,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
            )
        }
        return Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("homeTier")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .setContentIntent(pi)
            .setOngoing(true)
            .build()
    }

    private fun updateNotification(text: String) {
        val nm = getSystemService(NotificationManager::class.java)
        nm.notify(NOTIFICATION_ID, buildNotification(text))
    }
}

/**
 * TauriEventBus - 使用 evaluateJavascript 向 WebView 注入事件
 * 避免依赖 triggerCallback 静态变量，更符合 Tauri 2 移动端事件桥标准
 */
object TauriEventBus {
    private var webView: WebView? = null

    fun attach(wv: WebView) {
        webView = wv
        Log.d("TauriEventBus", "WebView attached")
    }

    fun detach() {
        webView = null
        Log.d("TauriEventBus", "WebView detached")
    }

    fun emit(event: String, payload: String) {
        val wv = webView ?: return
        wv.post {
            val js = """
                (function(){
                    if (window.__TAURI_INTERNALS__) {
                        window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
                            event: '$event',
                            payload: $payload
                        });
                    } else {
                        console.warn('Tauri internals not available');
                    }
                })();
            """.trimIndent()
            wv.evaluateJavascript(js, null)
        }
        Log.d("TauriEventBus", "Emitted event: $event, payload: $payload")
    }
}