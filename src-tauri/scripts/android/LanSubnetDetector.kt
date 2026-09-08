// LanSubnetDetector.kt - Android 物理 LAN 子网自动探测
package com.hometier.app

import android.content.Context
import android.net.wifi.WifiManager
import java.net.Inet4Address
import java.net.InetAddress
import java.net.NetworkInterface
import java.util.Collections
import kotlin.collections.mutableSetOf

object LanSubnetDetector {

    /**
     * 探测当前设备所在的物理 LAN 子网（/24）
     * 优先使用 WiFi 连接信息，兜底遍历网络接口
     * @return 去重后的 CIDR 列表，如 ["192.168.31.0/24"]
     */
    fun detect(context: Context): List<String> {
        android.util.Log.i("HomeTierVpn", "LanSubnetDetector.detect: 开始探测")
        val subnets = mutableSetOf<String>()

        // 1. 优先：WiFi 连接信息（最准确）
        try {
            val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as WifiManager
            val info = wifiManager.connectionInfo
            val ip = info.ipAddress
            val ssid = info.ssid
            android.util.Log.i("HomeTierVpn", "LanSubnetDetector: WiFi info - ip=$ip, ssid=$ssid")
            if (ip != 0) {
                val cidr = intToCidr(ip, 24)
                subnets.add(cidr)
                android.util.Log.i("HomeTierVpn", "LanSubnetDetector: WiFi 子网: $cidr")
            } else {
                android.util.Log.w("HomeTierVpn", "LanSubnetDetector: WiFi IP 为 0，可能缺少权限或未连接 WiFi")
            }
        } catch (e: Exception) {
            android.util.Log.e("HomeTierVpn", "LanSubnetDetector: WiFi 探测异常: ${e.message}", e)
            // 忽略，继续兜底
        }

        // 2. 兜底：遍历所有网络接口
        try {
            val interfaces = Collections.list(NetworkInterface.getNetworkInterfaces())
            android.util.Log.i("HomeTierVpn", "LanSubnetDetector: 发现 ${interfaces.size} 个网络接口")
            for (ni in interfaces) {
                android.util.Log.d("HomeTierVpn", "LanSubnetDetector: 接口 ${ni.name} - isUp=${ni.isUp}, isLoopback=${ni.isLoopback}")
                if (!ni.isUp || ni.isLoopback || isVirtualInterface(ni.name)) continue
                val addresses = Collections.list(ni.inetAddresses)
                for (addr in addresses) {
                    if (addr !is Inet4Address) continue
                    val host = addr.hostAddress
                    if (isSiteLocalIpv4(host)) {
                        val cidr = ipToCidr(host, 24)
                        subnets.add(cidr)
                        android.util.Log.i("HomeTierVpn", "LanSubnetDetector: 接口 ${ni.name} 子网: $cidr")
                    }
                }
            }
        } catch (e: Exception) {
            android.util.Log.e("HomeTierVpn", "LanSubnetDetector: 接口枚举异常: ${e.message}", e)
            // 忽略
        }

        val result = subnets.toList()
        android.util.Log.i("HomeTierVpn", "LanSubnetDetector: 最终结果: $result")
        return result
    }

    /** 判断是否为虚拟/隧道接口（需排除） */
    private fun isVirtualInterface(name: String): Boolean {
        val lower = name.lowercase()
        return lower.contains("tun") || lower.contains("docker") || lower.contains("veth") ||
               lower.contains("wg") || lower.contains("p2p") || lower.contains("virbr") ||
               lower.contains("br-") || lower.contains("tap") || lower.contains("vti") ||
               lower.contains("ip6tnl") || lower.contains("sit") || lower.contains("gre")
    }

    /** 判断是否为私有/站点本地 IPv4（10.x, 172.16-31.x, 192.168.x） */
    private fun isSiteLocalIpv4(host: String): Boolean {
        return host.startsWith("10.") ||
               host.startsWith("192.168.") ||
               host.startsWith("172.16.") || host.startsWith("172.17.") || host.startsWith("172.18.") ||
               host.startsWith("172.19.") || host.startsWith("172.20.") || host.startsWith("172.21.") ||
               host.startsWith("172.22.") || host.startsWith("172.23.") || host.startsWith("172.24.") ||
               host.startsWith("172.25.") || host.startsWith("172.26.") || host.startsWith("172.27.") ||
               host.startsWith("172.28.") || host.startsWith("172.29.") || host.startsWith("172.30.") ||
               host.startsWith("172.31.")
    }

    /** int (WifiManager 返回格式) → CIDR 字符串 */
    private fun intToCidr(ipInt: Int, prefix: Int): String {
        val octets = intArrayOf(
            (ipInt shr 24) and 0xFF,
            (ipInt shr 16) and 0xFF,
            (ipInt shr 8) and 0xFF,
            ipInt and 0xFF
        )
        // 掩码应用：/24 只保留前三段
        val masked = if (prefix == 24) intArrayOf(octets[0], octets[1], octets[2], 0) else octets
        return "${masked[0]}.${masked[1]}.${masked[2]}.${masked[3]}/$prefix"
    }

    /** 点分十进制字符串 → CIDR（应用 /24 掩码） */
    private fun ipToCidr(host: String, prefix: Int): String {
        val parts = host.split(".")
        if (parts.size != 4) return "$host/$prefix"
        val octets = parts.map { it.toInt() }.toIntArray()
        val masked = if (prefix == 24) intArrayOf(octets[0], octets[1], octets[2], 0) else octets
        return "${masked[0]}.${masked[1]}.${masked[2]}.${masked[3]}/$prefix"
    }
}