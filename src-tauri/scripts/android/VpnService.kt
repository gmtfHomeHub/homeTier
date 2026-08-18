package com.hometier.app

import android.app.Service
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.IBinder
import android.os.ParcelFileDescriptor
import android.util.Log
import org.json.JSONObject

/**
 * HomeTier VpnService
 * 
 * Creates a TUN interface and returns the file descriptor to the Rust layer
 * via Tauri event callback. Based on EasyTier's TauriVpnService.
 */
class HomeTierVpnService : VpnService() {

    private var vpnInterface: ParcelFileDescriptor? = null
    private val TAG = "HomeTierVpnService"

    companion object {
        private const val ACTION_START_VPN = "com.hometier.app.ACTION_START_VPN"
        private const val ACTION_STOP_VPN = "com.hometier.app.ACTION_STOP_VPN"
        private const val EXTRA_INTERFACE_NAME = "interface_name"
        private const val EXTRA_VIRTUAL_IP = "virtual_ip"
        private const val EXTRA_VIRTUAL_IP_CIDR = "virtual_ip_cidr"
        private const val EXTRA_MTU = "mtu"
        private const val EXTRA_ROUTES = "routes"
        private const val EXTRA_EXCLUDED_APPS = "excluded_apps"
        private const val EXTRA_DNS_SERVERS = "dns_servers"
    }

    override fun onBind(intent: Intent?): IBinder? {
        return null
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action
        when (action) {
            ACTION_START_VPN -> startVpn(intent!!)
            ACTION_STOP_VPN -> stopVpn()
        }
        return START_STICKY
    }

    private fun startVpn(intent: Intent) {
        val interfaceName = intent.getStringExtra(EXTRA_INTERFACE_NAME) ?: "tun0"
        val virtualIp = intent.getStringExtra(EXTRA_VIRTUAL_IP) ?: "10.144.144.1"
        val virtualIpCidr = intent.getIntExtra(EXTRA_VIRTUAL_IP_CIDR, 24)
        val mtu = intent.getIntExtra(EXTRA_MTU, 1500)
        val routesJson = intent.getStringExtra(EXTRA_ROUTES) ?: "[]"
        val excludedAppsJson = intent.getStringExtra(EXTRA_EXCLUDED_APPS) ?: "[]"
        val dnsServersJson = intent.getStringExtra(EXTRA_DNS_SERVERS) ?: "[]"

        val builder = Builder()
            .setSession(interfaceName)
            .setMtu(mtu)

        // Set virtual IP
        builder.addAddress(virtualIp, virtualIpCidr)
        // Also add IPv6 ULA
        builder.addAddress("fd00::1", 128)

        // Add routes
        try {
            val routes = JSONObject(routesJson).toJSONArray()
            for (i in 0 until routes.length()) {
                val route = routes.getString(i)
                val parts = route.split("/")
                if (parts.size == 2) {
                    val addr = parts[0]
                    val prefix = parts[1].toInt()
                    builder.addRoute(addr, prefix)
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse routes: $e")
        }

        // Add DNS servers
        try {
            val dnsServers = JSONObject(dnsServersJson).toJSONArray()
            for (i in 0 until dnsServers.length()) {
                builder.addDnsServer(dnsServers.getString(i))
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse DNS servers: $e")
        }

        // Exclude this app from VPN to prevent routing loop
        try {
            val excludedApps = JSONObject(excludedAppsJson).toJSONArray()
            for (i in 0 until excludedApps.length()) {
                val pkg = excludedApps.getString(i)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                    builder.addDisallowedApplication(pkg)
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse excluded apps: $e")
        }

        // Also exclude self
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            builder.addDisallowedApplication(packageName)
        }

        // Establish VPN interface
        try {
            vpnInterface = builder.establish()
            val fd = vpnInterface!!.detachFd()
            Log.i(TAG, "VPN established, fd=$fd")

            // Notify Rust layer via Tauri event
            sendTunFdEvent(fd, true, null)

        } catch (e: Exception) {
            Log.e(TAG, "Failed to establish VPN: $e")
            sendTunFdEvent(-1, false, e.message)
        }
    }

    private fun stopVpn() {
        try {
            vpnInterface?.close()
            vpnInterface = null
            Log.i(TAG, "VPN stopped")
            sendTunFdEvent(-1, true, null)
        } catch (e: Exception) {
            Log.e(TAG, "Error stopping VPN: $e")
        }
        stopSelf()
    }

    private fun sendTunFdEvent(fd: Int, success: Boolean, error: String?) {
        // Send event via Tauri's event system
        // This will be received by the frontend which calls set_tun_fd
        val event = JSONObject()
        try {
            event.put("type", "vpn:tun-ready")
            event.put("fd", fd)
            event.put("success", success)
            error?.let { event.put("error", it) }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to create event JSON: $e")
        }

        // Broadcast to Tauri WebView via custom intent action
        val broadcastIntent = Intent("com.hometier.app.VPN_EVENT")
        broadcastIntent.putExtra("event_data", event.toString())
        sendBroadcast(broadcastIntent)
    }

    override fun onRevoke() {
        super.onRevoke()
        Log.i(TAG, "VPN revoked by system")
        stopVpn()
    }
}