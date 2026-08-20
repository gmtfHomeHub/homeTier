//
//  BuilderHelper.swift
//  homeTier NetworkExtension
//
//  NEPacketTunnelNetworkSettings builder utilities
//

import Foundation
import NetworkExtension

// MARK: - Tunnel Configuration

struct TunnelConfiguration {
    let spaceId: String
    let virtualIPv4: String      // e.g., "10.144.144.1/24"
    let virtualIPv6: String?     // e.g., "fd00::1/128"
    let mtu: Int
    let routes: [String]         // CIDR strings for included routes
    let excludedApps: [String]   // Bundle identifiers to exclude
    let dnsServers: [String]
    let proxySettings: NEProxySettings?
}

// MARK: - Network Settings Builder

func buildTunnelNetworkSettings(from config: TunnelConfiguration) -> NEPacketTunnelNetworkSettings {
    // Parse IPv4
    let ipv4Parts = config.virtualIPv4.split(separator: "/")
    let ipv4Address = String(ipv4Parts[0])
    let ipv4Prefix = Int(ipv4Parts[1]) ?? 24
    let ipv4SubnetMask = IPv4CIDR.prefixToMask(ipv4Prefix)

    let settings = NEPacketTunnelNetworkSettings(
        tunnelRemoteAddress: ipv4Address,
        tunnelLocalAddress: ipv4Address,
        tunnelSubnetMask: ipv4SubnetMask
    )

    // MTU
    settings.mtu = validateMTU(config.mtu)

    // IPv6 (if provided)
    if let ipv6 = config.virtualIPv6 {
        let ipv6Parts = ipv6.split(separator: "/")
        let ipv6Address = String(ipv6Parts[0])
        let ipv6Prefix = Int(ipv6Parts[1]) ?? 128

        settings.ipv6Settings = NEIPv6Settings(
            addresses: [ipv6Address],
            networkPrefixLengths: [ipv6Prefix]
        )
    }

    // DNS
    let dnsServers = parseDNSServers(config.dnsServers)
    if !dnsServers.isEmpty {
        let dnsSettings = NEDNSSettings(servers: dnsServers)
        dnsSettings.matchDomains = ["hometier.local"]
        settings.dnsSettings = dnsSettings
    }

    // Included routes (only virtual network segment)
    let includedRoutes = buildIncludedRoutes(from: config.routes)
    settings.ipv4Settings?.includedRoutes = includedRoutes

    // IPv6 routes (if any)
    let ipv6Routes = config.routes.compactMap { NEIPv6Route(cidr: $0) }
    if !ipv6Routes.isEmpty {
        settings.ipv6Settings?.includedRoutes = ipv6Routes
    }

    // Exclude self (prevent routing loops)
    // Note: NEPacketTunnelProvider doesn't have direct excludedApplications
    // This is handled at the packet level by the Rust side

    // Proxy (if configured)
    if let proxy = config.proxySettings {
        settings.proxySettings = proxy
    }

    return settings
}

// MARK: - Builder for specific configurations

func buildDefaultSettings(
    spaceId: String,
    virtualIP: String,
    mtu: Int = 1500,
    routes: [String] = ["10.144.144.0/24"],
    dnsServers: [String] = ["10.144.144.1"]
) -> NEPacketTunnelNetworkSettings {
    let config = TunnelConfiguration(
        spaceId: spaceId,
        virtualIPv4: virtualIP,
        virtualIPv6: "fd00::1/128",
        mtu: mtu,
        routes: routes,
        excludedApps: ["com.hometier.app"],
        dnsServers: dnsServers,
        proxySettings: nil
    )
    return buildTunnelNetworkSettings(from: config)
}

// MARK: - Settings from App Group JSON

func buildSettingsFromAppGroupConfig() -> NEPacketTunnelNetworkSettings? {
    guard let configJson = readConfigFromAppGroup(),
          let data = configJson.data(using: .utf8),
          let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        return nil
    }

    let spaceId = json["space_id"] as? String ?? UUID().uuidString
    let virtualIP = json["virtual_ip"] as? String ?? "10.144.144.1/24"
    let mtu = json["mtu"] as? Int ?? 1500
    let routes = json["routes"] as? [String] ?? ["10.144.144.0/24"]
    let dnsServers = json["dns_servers"] as? [String] ?? ["10.144.144.1"]
    let excludedApps = json["excluded_apps"] as? [String] ?? ["com.hometier.app"]

    let config = TunnelConfiguration(
        spaceId: spaceId,
        virtualIPv4: virtualIP,
        virtualIPv6: json["virtual_ipv6"] as? String ?? "fd00::1/128",
        mtu: mtu,
        routes: routes,
        excludedApps: excludedApps,
        dnsServers: dnsServers,
        proxySettings: nil
    )
    return buildTunnelNetworkSettings(from: config)
}

// MARK: - Re-export AddressHelper types/functions

struct IPv4CIDR {
    let address: String
    let prefixLength: Int
    let subnetMask: String

    init?(cidr: String) {
        let parts = cidr.split(separator: "/")
        guard parts.count == 2,
              let prefix = Int(parts[1]),
              prefix >= 0 && prefix <= 32 else {
            return nil
        }
        self.address = String(parts[0])
        self.prefixLength = prefix
        self.subnetMask = Self.prefixToMask(prefix)
    }

    static func prefixToMask(_ prefix: Int) -> String {
        let mask = prefix == 0 ? 0 : (0xFFFFFFFF << (32 - prefix))
        let octets = [
            (mask >> 24) & 0xFF,
            (mask >> 16) & 0xFF,
            (mask >> 8) & 0xFF,
            mask & 0xFF
        ]
        return octets.map(String.init).joined(separator: ".")
    }
}

extension NEIPv4Route {
    convenience init?(cidr: String) {
        guard let parsed = IPv4CIDR(cidr: cidr) else { return nil }
        self.init(destinationAddress: parsed.address, subnetMask: parsed.subnetMask)
    }
}

func buildIncludedRoutes(from cidrs: [String]) -> [NEIPv4Route] {
    return cidrs.compactMap { NEIPv4Route(cidr: $0) }
}

func parseDNSServers(_ strings: [String]) -> [String] {
    return strings.filter { !$0.isEmpty }
}

func validateMTU(_ mtu: Int) -> Int {
    return max(576, min(9000, mtu))
}

func readConfigFromAppGroup() -> String? {
    let APP_GROUP_ID = "group.com.hometier.app"
    return UserDefaults(suiteName: APP_GROUP_ID)?.string(forKey: "VPNConfig")
}