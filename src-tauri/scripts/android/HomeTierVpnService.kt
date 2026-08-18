package com.hometier.app

import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.os.Bundle
import app.tauri.plugin.JSObject

/**
 * HomeTier VpnService
 *
 * Creates a TUN interface and passes the file descriptor back to the app
 * via triggerCallback. Based on EasyTier's TauriVpnService.
 *
 * The fd is an int and both Kotlin and Rust run in the same process on
 * Android, so the numeric fd is valid on the Rust side.
 */
class HomeTierVpnService : VpnService() {
    companion object {
        @JvmField var triggerCallback: (String, JSObject) -> Unit = { _, _ -> }
        @JvmField var self: HomeTierVpnService? = null
        @JvmField var ipv4Addr: String? = null
        @JvmField var routes: Array<String> = emptyArray()
        @JvmField var dns: String? = null

        const val IPV4_ADDR = "IPV4_ADDR"
        const val ROUTES = "ROUTES"
        const val DNS = "DNS"
        const val DISALLOWED_APPLICATIONS = "DISALLOWED_APPLICATIONS"
        const val MTU = "MTU"
    }

    private lateinit var vpnInterface: ParcelFileDescriptor

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        println("hometier vpn on start command ${intent?.getExtras()} $intent")
        val args = intent?.getExtras()
        ipv4Addr = args?.getString(IPV4_ADDR)
        routes = args?.getStringArray(ROUTES) ?: emptyArray()
        dns = args?.getString(DNS)

        vpnInterface = createVpnInterface(args)

        val eventData = JSObject()
        eventData.put("fd", vpnInterface.fd)
        triggerCallback("vpn_service_start", eventData)

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
            triggerCallback("vpn_service_stop", JSObject())
            vpnInterface.close()
        }
        clearStatus()
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
}
