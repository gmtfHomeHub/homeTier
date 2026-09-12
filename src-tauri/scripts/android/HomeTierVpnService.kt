// HomeTierVpnService.kt - Android VpnService implementation
package com.hometier.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import android.os.ParcelFileDescriptor
import android.util.Log

class HomeTierVpnService : VpnService() {

    companion object {
        @JvmField var self: HomeTierVpnService? = null
        @JvmField var ipv4Addr: String? = null
        @JvmField var routes: Array<String> = emptyArray()
        @JvmField var dns: String? = null
        @JvmField var intent: Intent? = null

        const val SPACE_ID = "spaceId"
        const val IPV4_ADDR = "ipv4Addr"
        const val ROUTES = "routes"
        const val DNS = "dns"
        const val DISALLOWED_APPLICATIONS = "disallowedApplications"
        const val MTU = "mtu"
        const val CHANNEL_ID = "hometier_vpn_channel"
        const val NOTIFICATION_ID = 1001
    }

    override fun onCreate() {
        super.onCreate()
        self = this
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        HomeTierVpnService.intent = intent
        val spaceId = intent?.getStringExtra(SPACE_ID) ?: ""
        ipv4Addr = intent?.getStringExtra(IPV4_ADDR)
        routes = intent?.getStringArrayExtra(ROUTES) ?: emptyArray()
        dns = intent?.getStringExtra(DNS)
        Log.i("HomeTierVpn", "onStartCommand spaceId=$spaceId ipv4Addr=$ipv4Addr")

        // Android 8+: startForegroundService 被调用后必须在 5s 内 startForeground，
        // 否则系统抛出 ForegroundServiceDidNotStartInTimeException 并杀进程。
        // VpnService 属 systemExempted 类型（manifest 已声明），与 VPN 前台服务兼容。
        val notification = buildNotification("homeTier VPN")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED)
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForeground(NOTIFICATION_ID, notification)
        }

        val pfd = createVpnInterface(intent?.extras)
        if (pfd != null) {
            // fd 注入：通过 TauriEventBus 发出 vpn:tun-ready 事件，
            // Rust 侧 setup.rs 的 listen("vpn:tun-ready") 与 JS 侧 mobileVpn.ts 双路接收后
            // 调用 set_tun_fd 命令将 fd 注入 EasyTier（双保险，幂等）。
            val fdNum = pfd.detachFd()
            TauriEventBus.emit("vpn:tun-ready", "{\"spaceId\":\"$spaceId\",\"fd\":$fdNum}")
        }

        return START_STICKY
    }

    override fun onRevoke() {
        super.onRevoke()
        Log.i("HomeTierVpn", "onRevoke - VPN revoked by system")
        ipv4Addr = null
        routes = emptyArray()
        dns = null
        intent = null
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
        val mtu = args?.getInt(MTU) ?: 1500
        val ipv4Addr = args?.getString(IPV4_ADDR) ?: "10.144.144.10/24"
        val dns = args?.getString(DNS)
        val routes = args?.getStringArray(ROUTES) ?: emptyArray()
        val disallowedApplications = args?.getStringArray(DISALLOWED_APPLICATIONS) ?: emptyArray()

        val ipParts = ipv4Addr.split("/")
        val (address, prefix) = if (ipParts.size == 2) {
            ipParts[0] to (ipParts[1].toIntOrNull() ?: 24)
        } else {
            Log.w("HomeTierVpn", "Invalid IP addr string: '$ipv4Addr', falling back to default")
            "10.144.144.10" to 24
        }

        fun base(): Builder = Builder()
            .setSession("HomeTierVpn")
            .setBlocking(false)
            .addAddress(address, prefix)
            .setMtu(mtu)

        // 主配置：IPv4 + IPv6(尽力而为) + DNS + 全部路由 + 排除应用
        // 每个可选项都用 runCatching 包裹，避免某一项不兼容导致 establish() 整体失败
        val full = base()
        runCatching { full.addAddress("fd00::1", 128) } // IPv6 失败不影响 IPv4
        dns?.let { runCatching { full.addDnsServer(it) } }
        for (route in routes) {
            val routeParts = route.split("/")
            when (routeParts.size) {
                2 -> runCatching { full.addRoute(routeParts[0], routeParts[1].toIntOrNull() ?: 24) }
                1 -> runCatching { full.addRoute(routeParts[0], 32) } // 单 IP 视为 /32
                else -> Log.w("HomeTierVpn", "Invalid route cidr string: '$route', skipping")
            }
        }
        // 仅应用前端传入的 excludedApps（默认空）。不硬编码排除自身：homeTier app 内的
        // HTTP 代理需经 TUN 访问虚拟 IP 转发应用请求。路由仅含虚拟 IP 网段，easytier peer
        // 通信走公网不在 routes，不会环路。
        for (app in disallowedApplications) runCatching { full.addDisallowedApplication(app) }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) full.setMetered(false)

        full.establish()?.let { return it }

        // 回退：完整配置 establish() 返回 null（如某些机型 IPv6/自定义路由不被接受）→
        // 用最小配置（仅 IPv4 + 自身可达路由 + 排除自身）重试，尽力保住 VPN 连接
        Log.w("HomeTierVpn", "establish() returned null with full config, retrying with minimal config")
        val minimal = base()
        runCatching { minimal.addRoute(address, prefix) }
        // minimal 回退配置同样不排除自身（理由同 full）
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) minimal.setMetered(false)

        return minimal.establish()
            ?: throw IllegalStateException("Failed to init VpnService (establish() returned null)")
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
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentIntent(pi)
            .setOngoing(true)
            .build()
    }
}