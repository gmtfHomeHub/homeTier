//
//  PacketTunnelProvider.swift
//  homeTier NetworkExtension
//
//  This file is adapted from EasyTier-iOS (GPL-3.0)
//  Original: https://github.com/EasyTier/EasyTier-iOS
//

import NetworkExtension
import os.log

// MARK: - Constants

let APP_BUNDLE_ID = "com.hometier.app"
let APP_GROUP_ID = "group.com.hometier.app"
let LOGGER_SUBSYSTEM = "com.hometier.app.tunnel"

// MARK: - FFI Declarations

@_silgen_name("init_logger")
func c_init_logger(
    _ path: UnsafePointer<CChar>?,
    _ level: UnsafePointer<CChar>?,
    _ subsystem: UnsafePointer<CChar>?,
    _ err: UnsafeMutablePointer<UnsafePointer<CChar>?>?
) -> CInt

@_silgen_name("clear_logger")
func c_clear_logger(_ err: UnsafeMutablePointer<UnsafePointer<CChar>?>?) -> CInt

@_silgen_name("run_network_instance")
func c_run_network_instance(
    _ cfg_str: UnsafePointer<CChar>?,
    _ err: UnsafeMutablePointer<UnsafePointer<CChar>?>?
) -> CInt

@_silgen_name("stop_network_instance")
func c_stop_network_instance() -> CInt

@_silgen_name("set_tun_fd")
func c_set_tun_fd(
    _ fd: CInt,
    _ err: UnsafeMutablePointer<UnsafePointer<CChar>?>?
) -> CInt

@_silgen_name("register_stop_callback")
func c_register_stop_callback(
    _ cb: (@convention(c) () -> Void)?,
    _ err: UnsafeMutablePointer<UnsafePointer<CChar>?>?
) -> CInt

@_silgen_name("register_running_info_callback")
func c_register_running_info_callback(
    _ cb: (@convention(c) () -> Void)?,
    _ err: UnsafeMutablePointer<UnsafePointer<CChar>?>?
) -> CInt

@_silgen_name("get_running_info")
func c_get_running_info(
    _ json: UnsafeMutablePointer<UnsafePointer<CChar>?>?,
    _ err: UnsafeMutablePointer<UnsafePointer<CChar>?>?
) -> CInt

@_silgen_name("get_latest_error_msg")
func c_get_latest_error_msg(
    _ msg: UnsafeMutablePointer<UnsafePointer<CChar>?>?,
    _ err: UnsafeMutablePointer<UnsafePointer<CChar>?>?
) -> CInt

@_silgen_name("free_string")
func c_free_string(_ s: UnsafePointer<CChar>?)

// MARK: - Logger

private let logger = OSLog(subsystem: LOGGER_SUBSYSTEM, category: "PacketTunnelProvider")

func logDebug(_ message: String) {
    os_log(.debug, log: logger, "%{public}@", message)
}

func logInfo(_ message: String) {
    os_log(.info, log: logger, "%{public}@", message)
}

func logError(_ message: String) {
    os_log(.error, log: logger, "%{public}@", message)
}

// MARK: - PacketTunnelProvider

class HomeTierTunnelProvider: NEPacketTunnelProvider {

    // MARK: - Properties

    private var stopCallbackRegistered = false
    private var runningInfoCallbackRegistered = false

    // MARK: - NEPacketTunnelProvider Overrides

    override func startTunnel(options: [String: NSObject]?, completionHandler: @escaping (Error?) -> Void) {
        logInfo("startTunnel called")

        // Initialize logger
        let logPath = getLogFilePath()?.path ?? ""
        let logLevel = "debug"
        var errPtr: UnsafePointer<CChar>?
        let result = c_init_logger(logPath, logLevel, LOGGER_SUBSYSTEM, &errPtr)
        if result != 0 {
            let errorMsg = extractError(errPtr)
            logError("Failed to init logger: \(errorMsg)")
            completionHandler(NSError(domain: "HomeTierVPN", code: -1, userInfo: [NSLocalizedDescriptionKey: errorMsg]))
            return
        }

        // Read configuration from App Group
        guard let configJson = readConfigFromAppGroup() else {
            let errorMsg = "Failed to read VPN config from App Group"
            logError(errorMsg)
            completionHandler(NSError(domain: "HomeTierVPN", code: -2, userInfo: [NSLocalizedDescriptionKey: errorMsg]))
            return
        }

        logInfo("Starting network instance with config: \(configJson)")

        // Start network instance
        var runErrPtr: UnsafePointer<CChar>?
        let runResult = c_run_network_instance(configJson, &runErrPtr)
        if runResult != 0 {
            let errorMsg = extractError(runErrPtr)
            logError("Failed to start network instance: \(errorMsg)")
            completionHandler(NSError(domain: "HomeTierVPN", code: -3, userInfo: [NSLocalizedDescriptionKey: errorMsg]))
            return
        }

        // Register callbacks
        registerCallbacks()

        // Apply network settings (this will trigger the fd callback)
        applyNetworkSettings(completionHandler: completionHandler)
    }

    override func stopTunnel(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        logInfo("stopTunnel called with reason: \(reason.rawValue)")

        // Stop network instance
        c_stop_network_instance()

        // Clear logger
        c_clear_logger(nil)

        completionHandler()
    }

    override func handleAppMessage(_ messageData: Data, completionHandler: ((Data?) -> Void)?) {
        logInfo("handleAppMessage received")

        guard let message = try? JSONSerialization.jsonObject(with: messageData) as? [String: Any],
              let action = message["action"] as? String else {
            completionHandler?(nil)
            return
        }

        var response: [String: Any] = [:]

        switch action {
        case "get_running_info":
            var jsonPtr: UnsafePointer<CChar>?
            var errPtr: UnsafePointer<CChar>?
            let result = c_get_running_info(&jsonPtr, &errPtr)
            if result == 0, let ptr = jsonPtr {
                let jsonStr = String(cString: ptr)
                c_free_string(ptr)
                if let data = jsonStr.data(using: .utf8),
                   let json = try? JSONSerialization.jsonObject(with: data) {
                    response["data"] = json
                }
            } else {
                response["error"] = extractError(errPtr)
            }

        case "get_latest_error":
            var msgPtr: UnsafePointer<CChar>?
            var errPtr: UnsafePointer<CChar>?
            let result = c_get_latest_error_msg(&msgPtr, &errPtr)
            if result == 0, let ptr = msgPtr {
                response["error"] = String(cString: ptr)
                c_free_string(ptr)
            } else {
                response["error"] = extractError(errPtr)
            }

        default:
            response["error"] = "Unknown action: \(action)"
        }

        if let responseData = try? JSONSerialization.data(withJSONObject: response) {
            completionHandler?(responseData)
        } else {
            completionHandler?(nil)
        }
    }

    // MARK: - Private Methods

    private func applyNetworkSettings(completionHandler: @escaping (Error?) -> Void) {
        // Build network settings
        let settings = buildTunnelNetworkSettings()

        // Set tunnel network settings - this will give us the packetFlow
        setTunnelNetworkSettings(settings) { [weak self] error in
            if let error = error {
                self?.logError("setTunnelNetworkSettings failed: \(error.localizedDescription)")
                completionHandler(error)
                return
            }

            self?.logInfo("Tunnel network settings applied successfully")

            // Get the TUN file descriptor
            self?.extractAndSetTunFd(completionHandler: completionHandler)
        }
    }

    private func buildTunnelNetworkSettings() -> NEPacketTunnelNetworkSettings {
        let settings = NEPacketTunnelNetworkSettings(
            tunnelRemoteAddress: "10.144.144.1",
            tunnelLocalAddress: "10.144.144.1",
            tunnelSubnetMask: "255.255.255.0"
        )

        // IPv6 (optional)
        settings.ipv6Settings = NEIPv6Settings(
            addresses: ["fd00::1"],
            networkPrefixLengths: [128]
        )

        // MTU
        settings.mtu = 1500

        // DNS
        let dnsSettings = NEDNSSettings(servers: ["10.144.144.1"])
        dnsSettings.matchDomains = ["hometier.local"]
        settings.dnsSettings = dnsSettings

        // Routes - only virtual network segment
        let route = NEIPv4Route(destinationAddress: "10.144.144.0", subnetMask: "255.255.255.0")
        settings.ipv4Settings?.includedRoutes = [route]

        // Exclude self
        settings.proxySettings = nil

        return settings
    }

    private func extractAndSetTunFd(completionHandler: @escaping (Error?) -> Void) {
        // Method 1: KVC on packetFlow
        if let fd = getTunFdFromPacketFlow() {
            logInfo("Got TUN fd from packetFlow KVC: \(fd)")
            setTunFdAndComplete(fd: fd, completionHandler: completionHandler)
            return
        }

        // Method 2: kern_control scan (fallback)
        if let fd = tunnelFileDescriptor() {
            logInfo("Got TUN fd from kern_control scan: \(fd)")
            setTunFdAndComplete(fd: fd, completionHandler: completionHandler)
            return
        }

        logError("Failed to obtain TUN file descriptor")
        completionHandler(NSError(domain: "HomeTierVPN", code: -4, userInfo: [NSLocalizedDescriptionKey: "Failed to obtain TUN file descriptor"]))
    }

    private func getTunFdFromPacketFlow() -> Int32? {
        // KVC: packetFlow.value(forKeyPath: "socket.fileDescriptor")
        // This uses private API but is the standard way in NE extensions
        let selector = NSSelectorFromString("socket")
        if packetFlow.responds(to: selector) {
            if let socket = packetFlow.perform(selector)?.takeUnretainedValue() as? NSObject {
                let fdSelector = NSSelectorFromString("fileDescriptor")
                if socket.responds(to: fdSelector) {
                    if let fdValue = socket.perform(fdSelector)?.takeUnretainedValue() as? NSNumber {
                        return fdValue.int32Value
                    }
                }
            }
        }
        return nil
    }

    private func tunnelFileDescriptor() -> Int32? {
        // Fallback: scan for kern_control (com.apple.net.utun_control)
        // This is the public API approach
        let maxFd = 1024
        var fd: Int32 = 0

        while fd < maxFd {
            var addr = sockaddr_ctl()
            var len = socklen_t(MemoryLayout<sockaddr_ctl>.size)

            let result = withUnsafeMutablePointer(to: &addr) { ptr in
                getsockname(fd, UnsafeMutablePointer<sockaddr>(OpaquePointer(ptr)), &len)
            }

            if result == 0 {
                if addr.sc_family == AF_SYSTEM,
                   addr.sysctl == AF_SYS_CONTROL,
                   addr.sc_id == 0 {
                    // Check if it's utun_control
                    var ctlInfo = ctl_info()
                    ctlInfo.ctl_id = 0
                    strncpy(&ctlInfo.ctl_name.0, "com.apple.net.utun_control", MemoryLayout.size(ofValue: ctlInfo.ctl_name))

                    var len2 = socklen_t(MemoryLayout<ctl_info>.size)
                    let result2 = getsockopt(fd, SYSPROTO_CONTROL, 2, &ctlInfo, &len2)
                    if result2 == 0 {
                        return fd
                    }
                }
            }
            fd += 1
        }
        return nil
    }

    private func setTunFdAndComplete(fd: Int32, completionHandler: @escaping (Error?) -> Void) {
        var errPtr: UnsafePointer<CChar>?
        let result = c_set_tun_fd(fd, &errPtr)
        if result != 0 {
            let errorMsg = extractError(errPtr)
            logError("Failed to set TUN fd: \(errorMsg)")
            completionHandler(NSError(domain: "HomeTierVPN", code: -5, userInfo: [NSLocalizedDescriptionKey: errorMsg]))
            return
        }

        logInfo("TUN fd \(fd) set successfully")
        completionHandler(nil)
    }

    private func registerCallbacks() {
        if !stopCallbackRegistered {
            c_register_stop_callback(stopCallback, nil)
            stopCallbackRegistered = true
        }

        if !runningInfoCallbackRegistered {
            c_register_running_info_callback(runningInfoCallback, nil)
            runningInfoCallbackRegistered = true
        }
    }

    // MARK: - C Callbacks

    private let stopCallback: @convention(c) () -> Void = {
        logInfo("Stop callback triggered from Rust")
        // Notify host app via Darwin notification
        notifyHostApp(event: "stopped")
    }

    private let runningInfoCallback: @convention(c) () -> Void = {
        logDebug("Running info callback triggered from Rust")
        notifyHostApp(event: "running_info")
    }

    private func notifyHostApp(event: String) {
        // Use Darwin notify for cross-process communication
        let notifyName = "com.hometier.app.vpn.\(event)" as CFString
        notify_post(notifyName)
    }

    // MARK: - Helpers

    private func getLogFilePath() -> URL? {
        guard let container = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP_ID) else {
            return nil
        }
        return container.appendingPathComponent("Library/Logs/homeTier_tunnel.log")
    }

    private func readConfigFromAppGroup() -> String? {
        guard let defaults = UserDefaults(suiteName: APP_GROUP_ID),
              let configJson = defaults.string(forKey: "VPNConfig") else {
            return nil
        }
        return configJson
    }

    private func extractError(_ errPtr: UnsafePointer<CChar>?) -> String {
        guard let ptr = errPtr else { return "Unknown error" }
        defer { c_free_string(ptr) }
        return String(cString: ptr)
    }
}

// MARK: - C Types for kern_control

import Darwin

private func strncpy(_ dest: UnsafeMutablePointer<CChar>, _ src: String, _ count: Int) {
    let srcCStr = src.cString(using: .utf8)!
    for i in 0..<min(count, srcCStr.count) {
        dest[i] = srcCStr[i]
    }
    if srcCStr.count < count {
        dest[srcCStr.count] = 0
    }
}