//
//  AddressHelper.swift
//  homeTier NetworkExtension
//
//  CIDR and routing utilities
//

import Foundation
import NetworkExtension

// MARK: - IPv4 CIDR Parsing

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

    static func maskToPrefix(_ mask: String) -> Int? {
        let octets = mask.split(separator: ".").compactMap { UInt8($0) }
        guard octets.count == 4 else { return nil }
        let maskValue = (UInt32(octets[0]) << 24) |
                       (UInt32(octets[1]) << 16) |
                       (UInt32(octets[2]) << 8) |
                        UInt32(octets[3])
        return maskValue == 0 ? 0 : 32 - maskValue.leadingZeros
    }
}

// MARK: - IPv6 CIDR Parsing

struct IPv6CIDR {
    let address: String
    let prefixLength: Int

    init?(cidr: String) {
        let parts = cidr.split(separator: "/")
        guard parts.count == 2,
              let prefix = Int(parts[1]),
              prefix >= 0 && prefix <= 128 else {
            return nil
        }

        self.address = String(parts[0])
        self.prefixLength = prefix
    }
}

// MARK: - NE Route Builders

extension NEIPv4Route {
    convenience init?(cidr: String) {
        guard let parsed = IPv4CIDR(cidr: cidr) else { return nil }
        self.init(destinationAddress: parsed.address, subnetMask: parsed.subnetMask)
    }
}

extension NEIPv6Route {
    convenience init?(cidr: String) {
        guard let parsed = IPv6CIDR(cidr: cidr) else { return nil }
        self.init(destinationAddress: parsed.address, networkPrefixLength: parsed.prefixLength)
    }
}

// MARK: - Network Settings Helpers

func parseIPv4Address(_ string: String) -> (address: String, prefixLength: Int)? {
    guard let cidr = IPv4CIDR(cidr: string) else { return nil }
    return (cidr.address, cidr.prefixLength)
}

func parseIPv6Address(_ string: String) -> (address: String, prefixLength: Int)? {
    guard let cidr = IPv6CIDR(cidr: string) else { return nil }
    return (cidr.address, cidr.prefixLength)
}

func buildIncludedRoutes(from cidrs: [String]) -> [NEIPv4Route] {
    return cidrs.compactMap { NEIPv4Route(cidr: $0) }
}

func buildExcludedRoutes(from cidrs: [String]) -> [NEIPv4Route] {
    return cidrs.compactMap { NEIPv4Route(cidr: $0) }
}

// MARK: - DNS Helpers

func parseDNSServers(_ strings: [String]) -> [String] {
    return strings.filter { !$0.isEmpty }
}

// MARK: - MTU Validation

func validateMTU(_ mtu: Int) -> Int {
    // Valid MTU range for TUN interfaces
    return max(576, min(9000, mtu))
}