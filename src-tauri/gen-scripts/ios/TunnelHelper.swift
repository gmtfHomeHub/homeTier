//
//  TunnelHelper.swift
//  homeTier NetworkExtension
//
//  Adapted from EasyTier-iOS (GPL-3.0)
//

import Foundation
import os.log

let LOGGER_SUBSYSTEM = "com.hometier.app.tunnel"
private let logger = OSLog(subsystem: LOGGER_SUBSYSTEM, category: "TunnelHelper")

func logDebug(_ message: String) {
    os_log(.debug, log: logger, "%{public}@", message)
}

func logInfo(_ message: String) {
    os_log(.info, log: logger, "%{public}@", message)
}

func logError(_ message: String) {
    os_log(.error, log: logger, "%{public}@", message)
}

// MARK: - FFI String Extraction

/// Extract a Rust-owned C string and convert to Swift String
/// The string must be freed with c_free_string after use
func extractRustString(_ ptr: UnsafePointer<CChar>?) -> String? {
    guard let ptr = ptr else { return nil }
    defer { c_free_string(ptr) }
    return String(cString: ptr)
}

/// Initialize Rust logger with file path
func initRustLogger(logPath: String, level: String = "info", subsystem: String = LOGGER_SUBSYSTEM) -> Bool {
    var errPtr: UnsafePointer<CChar>?
    let result = c_init_logger(logPath, level, subsystem, &errPtr)
    if result != 0 {
        if let error = extractRustString(errPtr) {
            logError("Failed to init Rust logger: \(error)")
        }
        return false
    }
    return true
}

// MARK: - Tunnel File Descriptor Detection

/// Find the utun file descriptor by scanning for kern_control
/// This is the fallback when KVC on packetFlow fails
func tunnelFileDescriptor() -> Int32? {
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
               addr.sysctl == AF_SYS_CONTROL {
                // Check if it's utun_control
                var ctlInfo = ctl_info()
                ctlInfo.ctl_id = 0
                let name = "com.apple.net.utun_control"
                withUnsafeMutableBytes(of: &ctlInfo.ctl_name) { bytes in
                    _ = name.utf8CString.copyBytes(to: bytes)
                }

                var len2 = socklen_t(MemoryLayout<ctl_info>.size)
                let result2 = getsockopt(fd, SYSPROTO_CONTROL, 2, &ctlInfo, &len2)
                if result2 == 0 {
                    logDebug("Found utun fd: \(fd)")
                    return fd
                }
            }
        }
        fd += 1
    }
    return nil
}

/// Set file descriptor to non-blocking mode
func setNonBlocking(_ fd: Int32) {
    let flags = fcntl(fd, F_GETFL)
    if flags >= 0 {
        fcntl(fd, F_SETFL, flags | O_NONBLOCK)
    }
}

// MARK: - Darwin Notify

/// Post a Darwin notification to communicate with host app
func notifyHostApp(_ event: String) {
    let notifyName = "com.hometier.app.vpn.\(event)" as CFString
    notify_post(notifyName)
}

/// Register for Darwin notifications from host app
func registerDarwinNotify(_ name: String, _ callback: @escaping () -> Void) {
    let notifyName = name as CFString
    let token = UnsafeMutableRawPointer(Unmanaged.passRetained(callback as AnyObject).toOpaque())
    notify_register_dispatch(notifyName, &token, DispatchQueue.main) { token in
        let callback = Unmanaged<AnyObject>.fromOpaque(token!).takeUnretainedValue() as! () -> Void
        callback()
    }
}

// MARK: - App Group Helpers

let APP_GROUP_ID = "group.com.hometier.app"

func getAppGroupContainer() -> URL? {
    return FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP_ID)
}

func getSharedLogPath() -> URL? {
    return getAppGroupContainer()?.appendingPathComponent("Library/Logs/homeTier_tunnel.log")
}

func writeConfigToAppGroup(_ configJson: String) -> Bool {
    guard let defaults = UserDefaults(suiteName: APP_GROUP_ID) else { return false }
    defaults.set(configJson, forKey: "VPNConfig")
    return defaults.synchronize()
}

func readConfigFromAppGroup() -> String? {
    return UserDefaults(suiteName: APP_GROUP_ID)?.string(forKey: "VPNConfig")
}

// MARK: - FFI Declarations (re-exported from PacketTunnelProvider)

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