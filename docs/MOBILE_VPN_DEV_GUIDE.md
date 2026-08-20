# homeTier 移动端 VPN 开发文档

> 版本：1.0
> 维护：移动端 VPN 模块开发团队
> 关联：docs/mobile_vpn.md (需求设计), docs/MOBILE_VPN_TEST_CHECKLIST.md (测试清单)

---

## 1. 架构概览

### 1.1 核心设计原则

```
┌─────────────────────────────────────────────────────────────┐
│                    Rust (统一协调层)                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ SpaceManager│  │EasyTierMgr  │  │ 事件监听器           │  │
│  │ connect/dis │  │start/stop   │  │ vpn:tun-ready       │  │
│  │ emit_vpn... │  │set_tun_fd   │  │ vpn:status-changed  │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
└─────────┼────────────────┼─────────────────────┼────────────┘
          │                │                     │
    ┌─────▼─────┐    ┌─────▼─────┐        ┌─────▼─────┐
    │  Android  │    │   iOS     │        │  Frontend │
    │ Kotlin    │    │  Swift    │        │  React/TS │
    │ VpnService│    │NEPacketTun│        │ 监听 vpn: │
    │ + Plugin  │    │  Provider │        │  state    │
    └───────────┘    └───────────┘        └───────────┘
```

**关键点**：
- **Rust 是中央协调者**：接收平台事件 → 注入 fd → 发送统一 `vpn:state` 事件
- **前端完全被动**：只监听 `vpn:state`，不直接处理 fd
- **fd 注入同步**：Kotlin/Swift 获得 fd → 通过 `evaluateJavascript` 发送事件 → Rust `set_tun_fd` → easytier 接管 TUN

---

## 2. Android 实现详解

### 2.1 文件结构

```
src-tauri/
├── scripts/
│   └── android/
│       ├── HomeTierVpnService.kt      # VpnService 实现
│       └── HomeTierVpnServicePlugin.kt # Tauri 插件
└── gen/android/app/src/main/java/com/hometier/app/
    ├── HomeTierVpnService.kt          # CI 注入
    ├── HomeTierVpnServicePlugin.kt    # CI 注入
    └── MainActivity.kt                # CI 注入 Plugin + WebView
```

### 2.2 关键类职责

| 类/组件 | 职责 |
|---------|------|
| `HomeTierVpnService` | 创建 TUN、前台服务、fd 回传 |
| `HomeTierVpnServicePlugin` | Tauri 命令桥 (`prepare_vpn`, `start_vpn`, `stop_vpn`) |
| `TauriEventBus` | `evaluateJavascript` 向 WebView 发送事件 |
| `MainActivity` | 持有 WebView，生命周期 attach/detach |

### 2.3 生命周期流程

```
用户点击连接
    │
    ▼
Frontend: prepareVpn() → invoke("plugin:hometiervpnservice|prepare_vpn")
    │
    ▼
Kotlin: VpnService.prepare() → 系统授权弹窗
    │
    ├─ 用户拒绝 → 返回 granted=false
    │
    └─ 用户允许 → 返回 granted=true
    │
    ▼
Frontend: connectSpace() → Rust: SpaceManager.connect()
    │
    ▼
Rust: EasyTierManager.start_network() → easytier 启动 P2P，等待 fd
    │
    ▼
Frontend: startVpn() → invoke("plugin:hometiervpnservice|start_vpn")
    │
    ▼
Kotlin: startService(Intent) → HomeTierVpnService.onStartCommand()
    │
    ├─ Builder 配置 IP/路由/MTU
    ├─ builder.addDisallowedApplication("com.hometier.app")  # 防死循环
    ├─ builder.establish() → ParcelFileDescriptor
    ├─ pfd.detachFd() → 获得 fd (int)
    ├─ startForeground() 显示通知 (Android 14+ 必须)
    └─ TauriEventBus.emit("vpn:tun-ready", {spaceId, fd})
    │
    ▼
Rust 事件监听器收到 → SpaceManager.set_tun_fd(spaceId, fd)
    │
    ▼
EasyTierManager.set_tun_fd() → sender.send(Some(fd))
    │
    ▼
easytier: create_dev_for_mobile(fd) → TUN 就绪
    │
    ▼
Rust 发送 vpn:state {state: "connected"} → Frontend 更新 UI
```

### 2.4 关键代码片段

**VpnService 启动并发送 fd**：
```kotlin
// HomeTierVpnService.kt
override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    currentSpaceId = args?.getString(SPACE_ID) ?: ""
    // ... 配置 builder ...
    vpnInterface = createVpnInterface(args)
    
    val eventData = JSONObject().apply {
        put("spaceId", currentSpaceId)
        put("fd", vpnInterface.fd)
    }
    TauriEventBus.emit("vpn:tun-ready", eventData.toString())
    return START_STICKY
}
```

**前台服务 (Android 14+)**：
```kotlin
// 必须在 onStartCommand 中调用
if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
    startForeground(NOTIFICATION_ID, notification, 
        ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED)
}
```

**Manifest 权限**：
```xml
<uses-permission android:name="android.permission.BIND_VPN_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED" />

<service
    android:name=".voip.HomeTierVpnService"
    android:permission="android.permission.BIND_VPN_SERVICE"
    android:exported="false"
    android:foregroundServiceType="systemExempted">
    <intent-filter>
        <action android:name="android.net.VpnService" />
    </intent-filter>
</service>
```

---

## 3. iOS 实现详解

### 3.1 文件结构

```
src-tauri/
├── easytier-ios-staticlib/           # Rust staticlib (FFI)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # FFI 入口
│       ├── instance.rs     # NetworkInstanceWrapper + json_to_easytier_config
│       ├── logger.rs       # os_log + App Group 文件日志
│       └── error.rs        # 错误处理
├── gen-scripts/ios/
│   ├── PacketTunnelProvider.swift   # NEPacketTunnelProvider
│   ├── TunnelHelper.swift         # fd 获取 (KVC + kern_control)
│   ├── BuilderHelper.swift        # NetworkSettings 构建
│   ├── AddressHelper.swift        # CIDR 工具
│   ├── inject_ne_target.py        # Xcode 项目注入脚本
│   ├── Info.plist                 # NE 扩展配置
│   └── entitlements.entitlements  # 权限
└── src-tauri/
    ├── Info.plist                 # 主 app 权限
    └── commands/ios_vpn.rs        # iOS VPN 命令
```

### 3.2 进程模型

```
┌─────────────────────────────────────────────────────────────┐
│  iOS 主 App 进程 (Tauri)                                      │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ Rust: SpaceManager + EasyTierManager (仅管理/状态查询)   │ │
│  └─────────────────────────────────────────────────────────┘ │
└───────────────────────────┬─────────────────────────────────┘
                            │ App Group (UserDefaults/文件)
                            │ + Darwin notify (跨进程事件)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  NE Extension 进程 (独立)                                     │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ Swift: PacketTunnelProvider                              │ │
│  │   - startTunnel → 读 App Group 配置                      │ │
│  │   - c_run_network_instance() → 启动 easytier (staticlib) │ │
│  │   - setTunnelNetworkSettings → 获得 packetFlow          │ │
│  │   - packetFlow KVC / kern_control 扫描 → 获得 fd        │ │
│  │   - c_set_tun_fd(fd) → 注入 easytier                    │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 3.3 关键数据流

```
1. Frontend 点击连接
   │
   ▼
2. Rust: start_ios_vpn(spaceId, configJson)
   │
   ├─ 写入 VPNConfig.json 到 App Group 容器
   ├─ 发送 "ios:start-vpn" 事件 (供 Swift 桥接监听)
   ▼
3. Swift: NETunnelProviderManager.startVPNTunnel() (由插件/桥接调用)
   │
   ▼
4. PacketTunnelProvider.startTunnel()
   │
   ├─ init_logger() → os_log + App Group 文件
   ├─ readConfigFromAppGroup() → 读取 VPNConfig.json
   ├─ c_run_network_instance(configJson) → 启动 easytier
   ├─ register_callbacks()
   ├─ applyNetworkSettings() → setTunnelNetworkSettings()
   │       │
   │       └─ completionHandler 中:
   │            ├─ getTunFdFromPacketFlow() (KVC: packetFlow.socket.fileDescriptor)
   │            └─ fallback: tunnelFileDescriptor() (扫描 kern_control)
   │
   ├─ c_set_tun_fd(fd) → 成功返回
   └─ completionHandler(nil)
   │
   ▼
5. easytier (staticlib): 收到 fd → create_dev_for_mobile() → TUN 就绪
   │
   ▼
6. 运行时回调:
   ├─ stop_callback → Darwin notify "com.hometier.app.vpn.stopped"
   └─ running_info_callback → Darwin notify "com.hometier.app.vpn.running_info"
```

### 3.4 FFI 函数签名

```rust
// easytier-ios-staticlib/src/lib.rs
#[no_mangle] pub extern "C" fn init_logger(
    path: *const c_char, level: *const c_char, 
    subsystem: *const c_char, err: *mut *const c_char
) -> c_int;

#[no_mangle] pub extern "C" fn run_network_instance(
    cfg_str: *const c_char, err: *mut *const c_char
) -> c_int;

#[no_mangle] pub extern "C" fn set_tun_fd(
    fd: c_int, err: *mut *const c_char
) -> c_int;

#[no_mangle] pub extern "C" fn register_stop_callback(
    cb: Option<extern "C" fn()>, err: *mut *const c_char
) -> c_int;

#[no_mangle] pub extern "C" fn register_running_info_callback(
    cb: Option<extern "C" fn()>, err: *mut *const c_char
) -> c_int;

// ... 更多函数
```

### 3.5 fd 获取策略 (TunnelHelper.swift)

```swift
// 方法 1: KVC (标准但用私有 API)
packetFlow.value(forKeyPath: "socket.fileDescriptor") as? Int32

// 方法 2: kern_control 扫描 (公开 API，兜底)
for fd in 0..1024 {
    var addr = sockaddr_ctl()
    getsockname(fd, &addr, &len)
    if addr.sc_family == AF_SYSTEM && addr.sysctl == AF_SYS_CONTROL {
        // 检查是否为 utun_control
        var ctlInfo = ctl_info()
        ctlInfo.ctl_name = "com.apple.net.utun_control"
        getsockopt(fd, SYSPROTO_CONTROL, 2, &ctlInfo, &len)
        if ctlInfo.ctl_id != 0 { return fd }
    }
}
```

---

## 4. CI/CD 构建流程

### 4.1 Android 构建

```yaml
# .github/workflows/release.yml
- name: Init Android project
  run: pnpm tauri android init
- name: Inject permissions & VpnService
  run: bash scripts/mobile-permissions.sh
- name: Build APK
  run: pnpm tauri android build --apk --target aarch64
```

### 4.2 iOS 构建

```yaml
# .github/workflows/release.yml
- name: Install mod_pbxproj
  run: pip3 install mod-pbxproj
- name: Build easytier-ios-staticlib
  run: |
    cd src-tauri
    cargo build --target aarch64-apple-ios --manifest-path easytier-ios-staticlib/Cargo.toml --release
- name: Inject NetworkExtension target
  run: |
    python3 src-tauri/gen-scripts/ios/inject_ne_target.py \
      src-tauri/gen/apple/HomeTier.xcodeproj \
      src-tauri/gen-scripts/ios
- name: Build iOS with NE
  run: |
    xcodebuild -project src-tauri/gen/apple/HomeTier.xcodeproj \
      -scheme HomeTier -configuration Release \
      -destination 'generic/platform=iOS' \
      CODE_SIGN_IDENTITY="" CODE_SIGNING_REQUIRED=NO CODE_SIGNING_ALLOWED=NO \
      build
```

### 4.3 签名要求

| 平台 | 要求 |
|------|------|
| Android | 无特殊要求 (debug keystore 即可) |
| iOS | **必须** Apple Developer 账号 + Network Extensions capability + Provisioning Profile |

---

## 5. 常见问题与排查

### 5.1 Android

| 问题 | 原因 | 解决 |
|------|------|------|
| `prepare_vpn` 总是返回 `need_prepare` | 未在 Manifest 注册 VpnService | 检查 `mobile-permissions.sh` 注入 |
| VPN 启动后立即断开 | 未调用 `startForeground` | Android 14+ 必须 `FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED` |
| fd 注入失败 | `sender.try_send` 失败 | 检查 easytier 是否已启动并等待 fd |
| 通知不显示 | NotificationChannel 未创建 | `createNotificationChannel()` 在 `onCreate` 中调用 |

### 5.2 iOS

| 问题 | 原因 | 解决 |
|------|------|------|
| `c_run_network_instance` 返回 -1 | JSON 配置解析失败 | 检查 `json_to_easytier_config` 字段映射 |
| 无法获得 fd | KVC 失败且 kern_control 扫描失败 | 确认 `packetFlow` 已在 `setTunnelNetworkSettings` 后可用 |
| NE 扩展不启动 | Entitlements 缺失 | 检查 `com.apple.developer.networking.networkextension` |
| App Group 读写失败 | Group ID 不匹配 | 主 App 和 NE 扩展必须同一 `group.com.hometier.app` |
| 静态库链接失败 | `libeasytier_ios_staticlib.a` 未找到 | CI 中确保 `cargo build --target aarch64-apple-ios` 先执行 |

### 5.3 通用

| 问题 | 排查步骤 |
|------|---------|
| 连接后无法访问虚拟网段 | 1. 检查路由表 `ip route` / `netstat -rn` <br> 2. 确认 `addDisallowedApplication` 未误拦截 <br> 3. 检查 easytier 日志 `peer_manager` 连接状态 |
| 空间切换失败 | 1. 检查 `SpaceManager.connect` 是否先 `stop_network` <br> 2. 确认 `cancel_tokens` 正确取消旧任务 |
| 前端状态不同步 | 1. 检查 `vpn:state` 事件是否发送 <br> 2. 确认 `SpaceManager.emit_vpn_state` 被调用 |

---

## 6. 调试技巧

### 6.1 开启详细日志

```rust
// Rust 端
crate::log::set_log_enabled(true);
// 或环境变量
RUST_LOG=debug,hometier=trace
```

```kotlin
// Android 端
adb logcat -s HomeTierVpn TauriEventBus *:V
```

```swift
// iOS 端
// Console.app -> 过滤 subsystem: com.hometier.app.tunnel
// 或 Xcode Devices -> Open Console
```

### 6.2 验证 fd 注入

```bash
# Android: 查看进程 fd
adb shell ls -l /proc/$(pidof com.hometier.app)/fd/ | grep tun

# iOS: 在 PacketTunnelProvider 中打印
logInfo("Got TUN fd: \(fd)")
```

### 6.3 验证网络连通性

```bash
# 连接成功后，在设备上 ping 虚拟网段内其他节点
ping 10.144.144.x

# 或用 curl 测试 HTTP 服务
curl http://10.144.144.x:port
```

---

## 7. 版本兼容性矩阵

| 组件 | 最低版本 | 推荐版本 | 备注 |
|------|---------|---------|------|
| Android | 8.0 (API 26) | 10+ | VpnService 基础支持 |
| Android (前台服务) | 14 (API 34) | 14+ | `FOREGROUND_SERVICE_SYSTEM_EXEMPTED` |
| iOS | 15.0 | 16+ | NetworkExtension 基础支持 |
| Rust | 1.95 (MSRV) | 1.97+ | easytier 要求 |
| easytier | 2.6.4 | 2.6.4 | vendored 版本 |
| Tauri | 2.x | 2.x | 移动端支持 |

---

## 8. 扩展指南

### 8.1 新增平台 (如 macOS Catalyst)

1. 实现 `TunProvider` trait (`src/mobile/mod.rs`)
2. 在 `get_tun_provider()` 中添加分支
3. 添加平台特定的 fd 获取逻辑
4. 更新 `SpaceManager` 和 `EasyTierManager` 的 mobile cfg

### 8.2 支持 IPv6

1. `TunConfig` 添加 `virtual_ipv6` 字段
2. Android: `builder.addAddress("fd00::1", 128)` 已支持
3. iOS: `settings.ipv6Settings` 已支持
4. easytier 配置 `enable_ipv6` flag

### 8.3 添加 DNS 代理

1. Android: `builder.addDnsServer("10.144.144.1")`
2. iOS: `NEDNSSettings(servers: ["10.144.144.1"])`
3. 前端配置 `dnsServers` 字段

---

## 9. 相关链接

- [需求设计文档](./mobile_vpn.md)
- [测试清单](./MOBILE_VPN_TEST_CHECKLIST.md)
- [EasyTier 官方 Android VpnService 参考](https://github.com/EasyTier/EasyTier/tree/main/tauri-plugin-vpnservice)
- [EasyTier-iOS 参考实现](https://github.com/EasyTier/EasyTier-iOS)
- [Android VpnService 文档](https://developer.android.com/reference/android/net/VpnService)
- [Apple NetworkExtension 文档](https://developer.apple.com/documentation/networkextension)
- [Tauri 移动端指南](https://tauri.app/v1/guides/distribution/mobile/)

---

*最后更新：2026-08-20*