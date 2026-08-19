# homeTier 移动端 VPN 服务需求、设计与开发说明书

> 适用版本：feat-serverization · easytier 库 ≥ 2.6.4 · Tauri ≥ 2.x
> 文档维护：移动端 VPN 模块是 homeTier 跨平台完整性的核心，本文是实施的总纲与索引

---

## 0. 文档目的与读者

- **目的**：明确 homeTier 移动端（Android、iOS）系统级 VPN 集成的需求边界、架构方案、关键实现点与工程风险，作为开发、测试、CI 维护的唯一参考。
- **读者**：Tauri/Rust/Swift 全栈开发工程师、CI 维护者、跨平台测试工程师。
- **与既有文档关系**：
  - 平台隔离现状在本文 §3 平台矩阵
  - Rust 侧抽象在 §5.2
  - Android 实现细节在 §6
  - iOS 实现细节在 §7
  - EasyTier-iOS 借鉴点见 §8（许可证与可复用清单）
  - CI/构建在 §10

---

## 1. 背景与现状

### 1.1 项目概述
homeTier 是基于 EasyTier 的去中心化组网工具，Tauri 2 + React/TS。已支持 Windows / macOS / Linux 桌面平台与 `--server`/`--daemon` 服务化模式，Android/iOS 编译通过但移动端 easytier 仅启动 P2P 层，缺少系统级 TUN，VPN 流量不通。

### 1.2 EasyTier 库移动端能力（本项目 vendored 版本已具备）
- `easytier::launcher::NetworkInstance::get_tun_fd_sender() -> mpsc::Sender<Option<i32>>`（launcher.rs:401）
- `launcher.rs:run_routine_for_mobile` 等待 fd channel 消息（mobile cfg = android/ios/macos-ne/ohos，build.rs cfg_aliases 定义）
- `virtual_nic.rs::create_dev_for_mobile(tun_fd: RawFd)` 用 `tun::Configuration::raw_fd(fd)` 接管外部分配的 tun
- iOS 时 `config.packet_information(false)`（utun fd 无 AF_DATA 头）
- 适用：Android（`/dev/net/tun` 不开放但有 `VpnService`）与 iOS（`/dev/net/tun` 不开放但有 `NEPacketTunnelProvider`）

### 1.3 官方实现参考
- **Android**：[EasyTier 官方 `tauri-plugin-vpnservice`](https://github.com/EasyTier/EasyTier/tree/main/tauri-plugin-vpnservice) —— Kotlin VpnService 子类 + Tauri 插件桥，路径已验证
- **iOS**：[EasyTier-iOS](https://github.com/EasyTier/EasyTier-iOS) —— SwiftUI + NEPacketTunnelProvider + Rust staticlib（FFI），App Store 已上架
  - 关键文件：`PacketTunnelProvider.swift`、`TunnelHelper.swift`、`AddressHelper.swift`、`BuilderHelper.swift`、`kern_control.h`、`Core/src/lib.rs`（FFI）

### 1.4 EasyTier-iOS fd 来源
```swift
// PacketTunnelProvider.swift 主路径（KVC 反射 packetFlow 内部 socket）：
guard let tunFd = self.packetFlow.value(forKeyPath: "socket.fileDescriptor") as? Int32
        ?? tunnelFileDescriptor() else { ... }
// tunnelFileDescriptor() 兜底：扫描 fd 0..1024 找 com.apple.net.utun_control kern_control
```

---

## 2. 功能需求

### 2.1 用户故事
| ID | 角色 | 描述 | 优先级 |
|----|------|------|--------|
| US-1 | 移动端用户 | 点击「连接」后，应用自动申请系统 VPN 授权，系统弹窗确认后建立 easytier 虚拟网络 | P0 |
| US-2 | 移动端用户 | 连接成功后，设备上其他应用（如浏览器、文件管理器）可访问 EasyTier 虚拟网段内的设备/服务 | P0 |
| US-3 | 移动端用户 | 断开连接时 VPN 自动卸载，前台通知消失 | P0 |
| US-4 | 移动端用户 | VPN 失败/异常时前端给出明确错误提示（含系统拒绝、隧道建立失败等） | P0 |
| US-5 | Android 用户 | 系统设置里可看到 homeTier VPN 配置文件（仅虚拟网段路由，不劫持默认流量） | P1 |
| US-6 | iOS 用户 | iOS 设置 → VPN 中可看到 homeTier 隧道，状态同步 | P0 |

### 2.2 功能范围

**包含**：
- Android `VpnService` 集成 + fd 注入到 Rust easytier
- iOS `NEPacketTunnelProvider` 集成 + Rust staticlib 中间层
- 跨平台统一 fd 注入抽象（`TunProvider` trait + `set_tun_fd` 命令）
- 前端连接流程改造（移动端特殊路径）
- 移动端权限/能力/权限申请
- CI 移动端构建（Android 完整，iOS NE 完整 + 签名）

**不包含**（明确排除）：
- 语音/屏幕共享移动端实现（保持 stub）
- iOS App Store 发布优化（代码签名可用 ad-hoc/entitlements 验证）
- 移动端开机自启
- 后台持续保活策略（iOS NE extension 生命周期由系统管理，不做特殊处理）

### 2.3 非功能需求
- **性能**：fd 注入到 TUN 联通目标 ≤ 3s（P95）
- **可靠性**：VPN 异常断开后 easytier 实例必须正确清理（无僵尸进程/资源泄漏）
- **安全**：
  - Android `addDisallowedApplication("com.hometier.app")` 防止死循环
  - 路由范围仅虚拟网段（用户决策）+ 用户配置的 proxy_cidrs，不劫持默认路由
  - iOS 不在 NEPacketTunnelProvider 写敏感信息到磁盘（仅写日志文件到 App Group）
- **可维护性**：fd 注入逻辑跨平台统一入口，新增 macOS Catalyst 等平台只需新增 `TunProvider` 实现
- **兼容性**：最低 Android 8.0（API 24）、iOS 15.0

---

## 3. 平台隔离矩阵

✓ 正常 · ✗ 完全不可用 · ◐ 部分可用/有缺陷 · N/A 不适用

| 核心功能 | Windows | macOS | Linux | Android | iOS | docker(server) |
|---|---|---|---|---|---|---|
| easytier 库集成 | ✓ | ✓ | ✓ | ◐ 缺 fd 注入（**本计划修复**） | ◐ 同（**本计划修复**） | N/A |
| easytier 二进制守护 | ✓ | ✓ | ✓ | ✗（隐藏更新按钮，stub upgrade） | ✗ | ✓ `--daemon` |
| HTTP 配置服务 (server 模式) | ✓ | ✓ | ✓ | ✗ cfg 隔离 | ✗ | ✓ `--server` |
| HTTP 内部代理（iframe） | ✓ | ✓ | ✓ | ✗ cfg 隔离（**AppBrowser 需降级**） | ✗ 同 | ✓ |
| 系统托盘 | ✓ | ✓ | ✓ | ✗ | ✗ | N/A |
| 窗口控制 | ✓ | ✓ | ✓ | ✗ | ✗ | N/A |
| 全局快捷键 | ✓ | ✓ | ✓ | ✗ 插件空壳（前端隐藏） | ✗ 同 | ✓ web 降级 |
| 开机自启 | ✓ | ✓ | ✓ | ✗ | ✗ | N/A |
| 应用自更新 | ✓ msi | ✓ dmg | ✓ deb+AppImage | ✗（隐藏入口） | ✗ | ✓ |
| 语音/屏幕共享 | ✓ | ✓ | ✓ | ✗ stub | ✗ stub | ✓ |
| 聊天/文件传输 | ✓ | ✓ | ✓ | ◐ broadcast/list stub | ◐ | ✓ |
| 日志系统 | ✓ | ✓ | ✓ | ✓ 内存+daemon 隐藏 | ✓ | ✓ |
| 机器 ID | ✓ | ✓ | ✓ | ✓ None | ✓ None | ✓ |
| config_store 服务化 | ✓ | ✓ | ✓ | ✓ 仅本机 DB | ✓ | ✓ 全功能 |
| 前端平台适配 | ✓ | ✓ | ✓ | ◐ 启动守卫/快捷键已降级；AppBrowser 需修 | ◐ | ✓ |
| **系统级 VPN** | N/A | N/A | N/A | **✓（本计划）** | **✓（本计划）** | N/A |

---

## 4. 总体架构

### 4.1 设计原则
1. **fd 注入抽象统一**：`TunProvider` trait 跨平台定义，Android/iOS 各自实现 `request_and_await_fd`
2. **Rust 单一职责**：easytier 库的 `NetworkInstance` 已支持 mobile fd 等待模式，本项目只需把 fd 注入到 `tun_fd_sender`
3. **零破坏**：桌面行为不变；移动端从 stub 升级为真实时不影响其他平台编译（`#[cfg]` 隔离）
4. **进程模型匹配平台**：Android fd 同进程注入（VpnService 跑在主 app 进程）；iOS NE 独立进程，必须重组 Rust 为 staticlib

### 4.2 Android 架构（同进程注入）

```
┌──────────────────────────────────────────────────────────┐
│  Android 主 app 进程（单进程）                              │
│  ┌────────────────────────────────────────────────┐      │
│  │ Tauri WebView (React/TS UI)                    │      │
│  │  └─ spacesStore.connect(spaceId)               │      │
│  └────────────────┬───────────────────────────────┘      │
│  ┌────────────────▼───────────────────────────────┐      │
│  │ Tauri Rust 主进程                                │      │
│  │  - SpaceManager.connect → EasyTierManager       │      │
│  │    .start_network → launcher_internal::         │      │
│  │    start_easytier → NetworkInstance::start()    │      │
│  │    （P2P 启动，run_routine_for_mobile 等待 fd） │      │
│  │  - 收到 Kotlin 事件后：set_tun_fd(spaceId, fd)  │      │
│  │    → EasyTierManager.set_tun_fd → sender.send() │      │
│  └────────────────┬───────────────────────────────┘      │
│  ┌────────────────▼───────────────────────────────┐      │
│  │ Kotlin VpnService (HomeTierVpnService)          │      │
│  │  - onStartCommand: Builder 配置 IP/路由/MTU     │      │
│  │  - establish() → ParcelFileDescriptor           │      │
│  │  - detachFd() → fd (int) → triggerCallback      │      │
│  └────────────────────────────────────────────────┘      │
└──────────────────────────────────────────────────────────┘
                         ↓ 系统 VPN 接口
              ┌──────────────────────────┐
              │ Android VpnService 系统层 │
              │ (tun0 接口 + 路由表)       │
              └──────────────────────────┘
```

**关键**：fd 是主 app 进程 fd，Rust 直接使用，**零 IPC**。

### 4.3 iOS 架构（NE Extension + staticlib 中间层）

```
┌─────────────────────────────────────┐    ┌─────────────────────────────────────┐
│  iOS 主 app 进程（Tauri）             │    │  iOS NE extension 进程（独立）       │
│  ┌───────────────────────────────┐  │    │  ┌───────────────────────────────┐  │
│  │ Tauri WebView (React/TS UI)   │  │    │  │ PacketTunnelProvider          │  │
│  │  └─ spacesStore.connect()     │  │    │  │  startTunnel(options, ...)    │  │
│  └─────────────┬─────────────────┘  │    │  │  setTunnelNetworkSettings     │  │
│  ┌─────────────▼─────────────────┐  │    │  │  fd = tunnelFileDescriptor()  │  │
│  │ Tauri Rust 主进程               │  │    │  │  set_tun_fd(fd)  // FFI       │  │
│  │  - 配置生成 (TOML→JSON)         │  │    │  └─────────────┬─────────────────┘  │
│  │  - 状态展示/UI 命令              │  │    │  ┌─────────────▼─────────────────┐  │
│  │  - NETunnelProviderManager     │──┼──┐ │  │ easytier-ios-staticlib (Rust) │  │
│  │  - App Group write/read        │  │  │ │  │  - run_network_instance       │  │
│  │  - Darwin notify 监听           │  │  │ │  │  - set_tun_fd → instance      │  │
│  └─────────────┬─────────────────┘  │  │ │  │  - get_running_info           │  │
└────────────────┼─────────────────────┘  │ │  │  - register_*_callback        │  │
                 ▼                        │ │  └───────────────────────────────┘  │
        ┌─────────────────┐              │ └─────────────┬─────────────────────────┘
        │  App Group       │◀─────────────┼────────────────┘
        │  UserDefaults    │              │
        │  + 共享文件       │              │
        └─────────────────┘              │
                                          │
        ┌─────────────────┐              │
        │ Darwin notify    │◀─────────────┘
        │ (双向事件)        │
        └─────────────────┘
```

**关键**：NE 进程独立跑 easytier，主 app 通过 App Group + Darwin notify 同步。

### 4.4 EasyTier Rust 库 mobile 等待 fd 流程

```
NetworkInstance::start()
    ↓
easy_routine 启动：
  ├─ P2P 层（peer_manager、connector）正常启动
  └─ tasks.spawn(run_routine_for_mobile)  ←── 等 fd
        ↓
   循环 recv tun_fd_receiver（mpsc channel）
        ↓
   收到 Some(fd) → Instance::setup_nic_ctx_for_mobile(nic_ctx, peer_mgr, peer_packet_receiver, fd)
        ↓
   virtual_nic::create_dev_for_mobile(tun_fd):
     - Configuration::default()
     - layer(Layer::L3)
     - iOS/macOS-ne: packet_information(false)
     - raw_fd(fd)                       ←── 接管外部分配的 tun
     - close_fd_on_drop(false)
     - tun::create(&config)
     - AsyncDevice::new → BiLock → TunStream
     - ifname = format!("tunfd_{}", fd)
```

**关键时序**：先 `NetworkInstance::start()`（P2P 层建立），再 `set_tun_fd()`（虚拟网卡就绪），后者是 easytier 库自己实现的等待机制。

---

## 5. 共享抽象与 Rust 改造

### 5.1 文件清单

| 新增/修改 | 路径 | 说明 |
|---------|------|------|
| 新增 | `src-tauri/src/mobile/mod.rs` | `TunProvider` trait 定义 |
| 新增 | `src-tauri/src/mobile/android.rs` | Android 实现（占位，fd 走 Tauri 事件桥） |
| 新增 | `src-tauri/src/mobile/ios.rs` | iOS 实现（占位，fd 走 App Group + Darwin notify） |
| 修改 | `src-tauri/src/easytier/mod.rs` | `EasyTierManager::set_tun_fd(instance_id, fd)` mobile impl |
| 修改 | `src-tauri/src/space/manager.rs` | mobile `connect` 异步协调 VPN；提供 `set_tun_fd` 入口 |
| 修改 | `src-tauri/src/lib.rs` | 注册 `set_tun_fd` tauri 命令（mobile cfg） |
| 新增 | `src-tauri/easytier-ios-staticlib/` | iOS FFI crate（仅 iOS build 时编译） |
| 新增 | `src-tauri/gen-scripts/ios/` | Swift NE 扩展源码、entitlements、Info.plist、pbxproj 注入脚本 |

### 5.2 `TunProvider` trait（跨平台抽象）

```rust
// src-tauri/src/mobile/mod.rs
#![cfg(any(target_os = "android", target_os = "ios"))]

use std::os::fd::RawFd;
use uuid::Uuid;

/// 跨平台系统级 VPN 抽象
pub trait TunProvider: Send + Sync {
    /// 准备系统授权（首次弹系统对话框）
    /// 返回 Ok(()) 表示系统已授权 VPN（用户已批准）
    async fn prepare(&self) -> Result<(), String>;

    /// 启动 VPN 并阻塞等待 fd 就绪（超时返回 Err）
    ///
    /// - Android: 触发 Kotlin VpnService → onStartCommand → establish → fd
    /// - iOS: 触发 NE startTunnel → setTunnelNetworkSettings → fd
    async fn start_and_await_fd(
        &self,
        space_id: Uuid,
        ipv4_addr: &str,
        routes: &[String],
        mtu: u32,
        excluded_app: Option<&str>,
    ) -> Result<RawFd, String>;

    /// 停止 VPN（清理系统 VPN 配置 + 通知 easytier）
    async fn stop(&self, space_id: Uuid) -> Result<(), String>;

    /// 健康检查
    fn is_active(&self, space_id: &Uuid) -> bool;
}
```

### 5.3 `EasyTierManager::set_tun_fd` mobile impl

```rust
// src-tauri/src/easytier/mod.rs (mobile cfg)
#[cfg(any(target_os = "android", target_os = "ios"))]
impl EasyTierManager {
    pub async fn set_tun_fd(&self, instance_id: &Uuid, fd: i32) -> Result<(), String> {
        let inst = self.instances.get(instance_id)
            .ok_or_else(|| "Instance not found".to_string())?;
        let ni = inst.instance.as_ref()
            .ok_or_else(|| "internal instance missing".to_string())?;
        let sender = ni.get_tun_fd_sender()
            .ok_or_else(|| "tun fd sender unavailable".to_string())?;
        sender.try_send(Some(fd))
            .map_err(|e| format!("send tun fd: {}", e))?;
        Ok(())
    }
}
```

### 5.4 tauri 命令注册

```rust
// src-tauri/src/lib.rs
#[cfg(any(target_os = "android", target_os = "ios"))]
#[tauri::command]
async fn set_tun_fd(
    space_id: String,
    fd: i32,
    space_manager: tauri::State<'_, Arc<SpaceManager>>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&space_id)
        .map_err(|e| format!("invalid space_id: {}", e))?;
    space_manager.set_tun_fd(id, fd).await
}
```

### 5.5 Tauri 事件（Kotlin/Swift → Rust → 前端）

```
Kotlin/Swift → Rust (事件):
  - "vpn:tun-ready" payload: { spaceId: String, fd: i32 }
  - "vpn:status-changed" payload: { spaceId, status: "preparing"|"ready"|"stopped"|"error", error?: string }

Rust → 前端 (事件透传):
  - "vpn:state" payload: { spaceId, state: "pending-vpn"|"connected"|"failed", error?: string }
```

### 5.6 iOS staticlib crate（`easytier-ios-staticlib`）

**Cargo.toml**：
```toml
[package]
name = "easytier-ios-staticlib"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["staticlib"]

[dependencies]
easytier = { path = "../resources/easytier_lib/easytier", features = ["wireguard","websocket","tun","socks5","kcp","quic","zstd","macos-ne"] }
tokio = { version = "1", features = ["rt-multi-thread","macros","sync"] }
tracing = "0.1"
tracing-oslog = "0.3"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
once_cell = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**src/lib.rs**（参考 EasyTier-iOS `Core/src/lib.rs`）：暴露 FFI：
```rust
#[no_mangle] pub extern "C" fn init_logger(path: *const c_char, level: *const c_char, subsystem: *const c_char, err: *mut *const c_char) -> c_int;
#[no_mangle] pub extern "C" fn clear_logger(err: *mut *const c_char) -> c_int;
#[no_mangle] pub extern "C" fn run_network_instance(cfg_str: *const c_char, err: *mut *const c_char) -> c_int;
#[no_mangle] pub extern "C" fn stop_network_instance() -> c_int;
#[no_mangle] pub extern "C" fn set_tun_fd(fd: c_int, err: *mut *const c_char) -> c_int;
#[no_mangle] pub extern "C" fn register_stop_callback(cb: Option<extern "C" fn()>, err: *mut *const c_char) -> c_int;
#[no_mangle] pub extern "C" fn register_running_info_callback(cb: Option<extern "C" fn()>, err: *mut *const c_char) -> c_int;
#[no_mangle] pub extern "C" fn get_running_info(json: *mut *const c_char, err: *mut *const c_char) -> c_int;
#[no_mangle] pub extern "C" fn get_latest_error_msg(msg: *mut *const c_char, err: *mut *const c_char) -> c_int;
#[no_mangle] pub extern "C" fn free_string(s: *const c_char);
```

**关键点**：
- 全局单例 `static INSTANCE: Lazy<Arc<Mutex<Option<NetworkInstance>>>>`
- `set_tun_fd` 直接拿 `inst.get_tun_fd_sender()` 发 fd
- 日志写到 App Group 容器路径，主 app 可读

---

## 6. Android 实现细节

### 6.1 关键文件

| 文件 | 路径（CI 注入目标） | 说明 |
|------|-------------------|------|
| `HomeTierVpnService.kt` | `src-tauri/gen/android/app/src/main/java/com/hometier/app/voip/` | VpnService 子类 |
| `MainActivityExt.kt` | 同上 | MainActivity 扩展方法（prepareVpn/startVpn） |
| `AndroidManifest.xml` 片段 | `src-tauri/gen/android/app/src/main/AndroidManifest.xml` | service 声明 + BIND_VPN_SERVICE 权限 |

### 6.2 `HomeTierVpnService.kt` 模板

```kotlin
package com.hometier.app.voip

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log

class HomeTierVpnService : VpnService() {
    companion object {
        const val ACTION_START = "com.hometier.app.START_VPN"
        const val ACTION_STOP = "com.hometier.app.STOP_VPN"
        const val EXTRA_SPACE_ID = "space_id"
        const val EXTRA_IPV4_ADDR = "ipv4_addr"
        const val EXTRA_ROUTES = "routes"  // String[] CIDR
        const val EXTRA_MTU = "mtu"
        const val EXTRA_EXCLUDED_APPS = "excluded_apps"
        const val CHANNEL_ID = "homeTierVpn"
        const val NOTIFICATION_ID = 1001

        // 启动入口（MainActivity 调用）
        @JvmStatic
        fun start(
            context: android.content.Context,
            spaceId: String,
            ipv4Addr: String,
            routes: Array<String>,
            mtu: Int,
            excludedApps: Array<String> = emptyArray()
        ) {
            val intent = Intent(context, HomeTierVpnService::class.java).apply {
                action = ACTION_START
                putExtra(EXTRA_SPACE_ID, spaceId)
                putExtra(EXTRA_IPV4_ADDR, ipv4Addr)
                putExtra(EXTRA_ROUTES, routes)
                putExtra(EXTRA_MTU, mtu)
                putExtra(EXTRA_EXCLUDED_APPS, excludedApps)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }
    }

    private var pfd: ParcelFileDescriptor? = null
    private var spaceId: String = ""

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        startForeground(NOTIFICATION_ID, buildNotification("准备连接…"))
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopVpn()
                return START_NOT_STICKY
            }
            ACTION_START -> {
                spaceId = intent.getStringExtra(EXTRA_SPACE_ID) ?: ""
                val ipv4 = intent.getStringExtra(EXTRA_IPV4_ADDR) ?: ""
                val routes = intent.getStringArrayExtra(EXTRA_ROUTES) ?: emptyArray()
                val mtu = intent.getIntExtra(EXTRA_MTU, 1500)
                val excluded = intent.getStringArrayExtra(EXTRA_EXCLUDED_APPS) ?: emptyArray()
                establish(spaceId, ipv4, routes, mtu, excluded)
            }
        }
        return START_STICKY
    }

    private fun establish(spaceId: String, ipv4: String, routes: Array<String>, mtu: Int, excludedApps: Array<String>) {
        val builder = Builder()
            .setSession("homeTier VPN")
            .setMtu(mtu)

        // IPv4 地址（必须含前缀长度）
        builder.addAddress(ipv4.split("/")[0], ipv4.split("/")[1].toInt())

        // 路由：仅虚拟网段 + 用户配置的 proxy_cidrs
        routes.forEach { cidr ->
            val parts = cidr.split("/")
            builder.addRoute(parts[0], parts[1].toInt())
        }

        // 排除自己（防止死循环）
        excludedApps.forEach { pkg ->
            try {
                builder.addDisallowedApplication(pkg)
            } catch (e: Exception) {
                Log.w("HomeTierVpn", "addDisallowedApplication($pkg) failed: $e")
            }
        }

        val newPfd = builder.establish()
        if (newPfd == null) {
            notifyError("VPN 接口建立失败")
            stopSelf()
            return
        }

        // 关闭旧的
        pfd?.close()
        pfd = newPfd

        // detachFd：fd 所有权转移给 Rust（close_fd_on_drop(false) 对应）
        val fd = newPfd.detachFd()

        // 通过 Tauri 事件桥把 fd 传给 Rust（同进程，可直接 triggerCallback）
        try {
            val eventData = org.json.JSONObject().apply {
                put("spaceId", spaceId)
                put("fd", fd)
            }
            // 触发 Tauri 事件（Kotlin → Rust 桥）
            triggerCallback("vpn:tun-ready", eventData.toString())
            updateNotification("已连接")
        } catch (e: Exception) {
            Log.e("HomeTierVpn", "triggerCallback failed: $e")
            notifyError("fd 注入失败: ${e.message}")
            stopSelf()
            return
        }
    }

    private fun stopVpn() {
        pfd?.close()
        pfd = null
        try {
            triggerCallback("vpn:status-changed", """{"spaceId":"$spaceId","status":"stopped"}""")
        } catch (_: Exception) {}
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            stopForeground(STOP_FOREGROUND_REMOVE)
        } else {
            @Suppress("DEPRECATION") stopForeground(true)
        }
        stopSelf()
    }

    override fun onRevoke() {
        stopVpn()
    }

    override fun onDestroy() {
        stopVpn()
        super.onDestroy()
    }

    private fun triggerCallback(event: String, payload: String) {
        // 通过 Tauri 移动端事件桥（具体实现见 §6.3 MainActivityExt）
        TauriEventBus.emit(event, payload)
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

    private fun notifyError(msg: String) {
        Log.e("HomeTierVpn", msg)
        updateNotification("VPN 错误: $msg")
    }
}
```

### 6.3 `TauriEventBus` 桥（MainActivity 内静态注册）

```kotlin
// MainActivity.kt 中追加（CI 注入 patch）
object TauriEventBus {
    private var webView: android.webkit.WebView? = null

    fun attach(wv: android.webkit.WebView) { webView = wv }

    @JavascriptInterface
    fun emit(event: String, payload: String) {
        webView?.post {
            webView?.evaluateJavascript("""
                (function(){
                    if (window.__TAURI_INTERNALS__) {
                        window.__TAURI_INTERNALS__.invoke('plugin:event|listen', {
                            event: '$event',
                            payload: $payload
                        });
                    }
                })();
            """.trimIndent(), null)
        }
    }
}
```

> **说明**：上述 `emit` 用 `evaluateJavascript` 注入到 Tauri WebView 内部执行 invoke，等同于前端 `listen()` 注册的回调。这是 Tauri 2 移动端事件桥的标准方式（Kotlin 侧无官方插件情况下）。若改用 tauri-plugin-event 自定义命令则更干净，但当前架构改动更大，本期先用 `evaluateJavascript` 方案。

### 6.4 `MainActivityExt.kt`

```kotlin
package com.hometier.app

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import com.hometier.app.voip.HomeTierVpnService

object MainActivityExt {
    const val VPN_REQUEST_CODE = 9001

    /** 前端调用：检查并请求 VPN 授权，返回 granted bool */
    @JvmStatic
    fun prepareVpn(activity: Activity): Boolean {
        val intent = VpnService.prepare(activity)
        if (intent != null) {
            activity.startActivityForResult(intent, VPN_REQUEST_CODE)
            return false  // 等待用户授权
        }
        return true  // 已授权
    }

    @JvmStatic
    fun startVpn(
        context: android.content.Context,
        spaceId: String,
        ipv4Addr: String,
        routes: Array<String>,
        mtu: Int,
        excludedApps: Array<String> = emptyArray()
    ) {
        HomeTierVpnService.start(context, spaceId, ipv4Addr, routes, mtu, excludedApps)
    }

    @JvmStatic
    fun stopVpn(context: android.content.Context) {
        val intent = Intent(context, HomeTierVpnService::class.java).apply {
            action = HomeTierVpnService.ACTION_STOP
        }
        context.startService(intent)
    }
}
```

### 6.5 AndroidManifest.xml 片段

```xml
<!-- 在 <application> 标签内 -->
<service
    android:name=".voip.HomeTierVpnService"
    android:permission="android.permission.BIND_VPN_SERVICE"
    android:exported="false"
    android:foregroundServiceType="systemExempted">
    <intent-filter>
        <action android:name="android.net.VpnService" />
    </intent-filter>
</service>

<!-- 在 <manifest> 顶层 -->
<uses-permission android:name="android.permission.BIND_VPN_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<!-- API 34+ 需要 -->
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED" />
```

### 6.6 `mobile-permissions.sh` 扩展（CI 注入）

```bash
#!/bin/bash
# 追加到现有 scripts/mobile-permissions.sh

ANDROID_MAIN="${ANDROID_MANIFEST_DIR:-src-tauri/gen/android/app/src/main/java/com/hometier/app}"
ANDROID_MANIFEST="src-tauri/gen/android/app/src/main/AndroidManifest.xml"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# 注入 VpnService.kt
mkdir -p "$ANDROID_MAIN/voip"
if [ ! -f "$ANDROID_MAIN/voip/HomeTierVpnService.kt" ]; then
    cp "$SCRIPT_DIR/android/HomeTierVpnService.kt" "$ANDROID_MAIN/voip/"
    echo "[mobile-permissions] Injected HomeTierVpnService.kt"
fi

# 注入 MainActivityExt.kt
if [ ! -f "$ANDROID_MAIN/MainActivityExt.kt" ]; then
    cp "$SCRIPT_DIR/android/MainActivityExt.kt" "$ANDROID_MAIN/"
    echo "[mobile-permissions] Injected MainActivityExt.kt"
fi

# 合并 AndroidManifest 片段（幂等）
if [ -f "$ANDROID_MANIFEST" ]; then
    if ! grep -q 'HomeTierVpnService' "$ANDROID_MANIFEST"; then
        node -e "
const fs=require('fs');
let c=fs.readFileSync('$ANDROID_MANIFEST','utf8');
const serviceBlock=\`<service
    android:name=\\\".voip.HomeTierVpnService\\\"
    android:permission=\\\"android.permission.BIND_VPN_SERVICE\\\"
    android:exported=\\\"false\\\"
    android:foregroundServiceType=\\\"systemExempted\\\">
    <intent-filter>
        <action android:name=\\\"android.net.VpnService\\\" />
    </intent-filter>
</service>\`;
c=c.replace('</application>', serviceBlock + '\n</application>');
const perms=\`<uses-permission android:name=\\\"android.permission.BIND_VPN_SERVICE\\\" />
<uses-permission android:name=\\\"android.permission.FOREGROUND_SERVICE\\\" />
<uses-permission android:name=\\\"android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED\\\" />\`;
c=c.replace('</manifest>', perms + '\n</manifest>');
fs.writeFileSync('$ANDROID_MANIFEST',c);
"
        echo "[mobile-permissions] Injected VpnService manifest entries"
    fi
fi
```

### 6.7 前端连接流程（Android）

```typescript
// src/utils/api/vpn.ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isMobile } from "@/utils/platform";

export async function prepareVpn(): Promise<boolean> {
  if (!isMobile()) return true;
  return await invoke("plugin:android-vpn|prepare_vpn");
}

export async function startVpn(opts: {
  spaceId: string;
  ipv4Addr: string;
  routes: string[];
  mtu: number;
}): Promise<void> {
  if (!isMobile()) return;
  await invoke("plugin:android-vpn|start_vpn", opts);
}

export async function setTunFd(spaceId: string, fd: number): Promise<void> {
  await invoke("set_tun_fd", { spaceId, fd });
}

// 监听 VPN 事件
export async function onVpnReady(cb: (spaceId: string, fd: number) => void) {
  return await listen<{ spaceId: string; fd: number }>("vpn:tun-ready", (e) => cb(e.payload.spaceId, e.payload.fd));
}

export async function onVpnStatus(cb: (spaceId: string, status: string, error?: string) => void) {
  return await listen<{ spaceId: string; status: string; error?: string }>(
    "vpn:status-changed",
    (e) => cb(e.payload.spaceId, e.payload.status, e.payload.error)
  );
}
```

### 6.8 `spacesStore.connect` Android 分支

```typescript
// src/stores/spacesStore.ts (伪代码)
async function connect(spaceId: string) {
  const space = getSpace(spaceId);
  // 1. 调用后端 start_network（启动 P2P，等待 fd）
  await invoke("connect_space", { spaceId });
  if (isMobile() && isAndroid()) {
    // 2. 准备 VPN 授权
    const granted = await prepareVpn();
    if (!granted) {
      // 等用户授权回调（MainActivity onActivityResult 处理）
      return { state: "pending-auth" };
    }
    // 3. 启动 VpnService
    const ipv4 = space.network.ipv4; // "10.144.144.1/24"
    const routes = ["/24 from ipv4"]; // + user proxy_cidrs
    await startVpn({ spaceId, ipv4Addr: ipv4, routes, mtu: 1500 });
    // 4. 等待 vpn:tun-ready 事件 → 自动 setTunFd
    //    （在 App.tsx 注册全局监听，转发到 setTunFd）
    return { state: "pending-vpn" };
  }
  return { state: "connected" };
}
```

---

## 7. iOS 实现细节

### 7.1 关键文件

| 文件 | 路径 | 说明 |
|------|------|------|
| `PacketTunnelProvider.swift` | `src-tauri/gen-scripts/ios/PacketTunnelProvider.swift` | NEPacketTunnelProvider 子类（**Copy from EasyTier-iOS**） |
| `TunnelHelper.swift` | 同上 | kern_control fd 扫描、setNonBlocking、App Group 日志、FFI 字符串提取 |
| `AddressHelper.swift` | 同上 | CIDR/路由工具 |
| `BuilderHelper.swift` | 同上 | NEPacketTunnelNetworkSettings 构造 |
| `Info.plist` | 同上 | NE extension 配置 |
| `entitlements.entitlements` | 同上 | NE + App Group |
| `kern_control.h` | 同上 | utun kern_control 接口 |
| `easytier_ios.h` | 同上 | FFI 函数声明 |
| `inject_ne_target.py` | `src-tauri/gen-scripts/ios/` | 修改 Xcode pbxproj 添加 NE target |
| `build.rs` hook | `src-tauri/build.rs` | iOS build 前调用 `inject_ne_target.py` |

### 7.2 `PacketTunnelProvider.swift`（复制并适配）

> **策略**：从 [EasyTier-iOS/EasyTierNetworkExtension/PacketTunnelProvider.swift](https://github.com/EasyTier/EasyTier-iOS/blob/main/EasyTierNetworkExtension/PacketTunnelProvider.swift) 直接复制，修改：
> - `APP_BUNDLE_ID` 常量 → `com.hometier.app`
> - `APP_GROUP_ID` → `group.com.hometier.app`
> - `Logger` subsystem → `com.hometier.app.tunnel`
> - 主类引用名 → `HomeTierTunnelProvider`（避免与可能的 EasyTierShared 符号冲突）

其余逻辑（startTunnel、stopTunnel、applyNetworkSettings、tunnelFileDescriptor kern_control 扫描、register_*_callback、notifyHostAppError Darwin notify）**保持原样**。

### 7.3 `Info.plist` (extension)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>NEMachServiceName</key>
    <string>com.hometier.app.tunnel</string>
    <key>NSExtension</key>
    <dict>
        <key>NSExtensionPointIdentifier</key>
        <string>com.apple.networkextension.packet-tunnel</string>
        <key>NSExtensionPrincipalClass</key>
        <string>$(PRODUCT_MODULE_NAME).HomeTierTunnelProvider</string>
    </dict>
</dict>
</plist>
```

### 7.4 `entitlements.entitlements`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.developer.networking.networkextension</key>
    <array>
        <string>packet-tunnel-provider</string>
    </array>
    <key>com.apple.security.application-groups</key>
    <array>
        <string>group.com.hometier.app</string>
    </array>
</dict>
</plist>
```

### 7.5 `inject_ne_target.py`（Xcode 工程注入脚本）

```python
#!/usr/bin/env python3
"""
向 Tauri 生成的 Xcode 工程注入 NetworkExtension target。
输入: src-tauri/gen/apple/HomeTier.xcodeproj/project.pbxproj
输出: 添加 NE target + build phases + 链接 staticlib
"""
import sys, re
from pathlib import Path

def inject(pbxproj_path: Path, ne_dir: Path):
    text = pbxproj_path.read_text()
    # 简化版：解析 PBX 节点并插入 NE target
    # 实际实现使用 mod_pbxproj 或手写 regex
    # 参考：https://github.com/kronenthaler/mod-pbxproj
    # ... (此处略，实际工程实现需 200+ 行)
    pass

if __name__ == "__main__":
    inject(Path(sys.argv[1]), Path(sys.argv[2]))
```

> **实现注意**：Xcode pbxproj 是 OpenStep plist 格式，手写解析风险高。**强烈建议**用 [`mod-pbxproj`](https://github.com/kronenthaler/mod-pbxproj) Python 库或 [`xcodeproj`](https://github.com/CocoaPods/Xcodeproj) Ruby 库（社区成熟）。

### 7.6 `build.rs` 集成

```rust
// src-tauri/build.rs (iOS target 检测)
fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "ios" {
        let script = std::env::var("HOME").map(|h| format!("{}/.cargo/bin/inject_ne_target", h)).unwrap_or_default();
        // 调用 Python 脚本注入 NE target 到 Xcode 工程
        println!("cargo:warning=Running iOS NE target injection (if applicable)");
    }
}
```

### 7.7 iOS 运行时：Tauri 主进程 → NE

```rust
// src-tauri/src/lib.rs (iOS cfg)
#[cfg(target_os = "ios")]
#[tauri::command]
async fn start_ios_vpn(
    space_id: String,
    config_json: String,  // EasyTier TOML → JSON 序列化
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    use tauri::Manager;
    // 1. 写 App Group UserDefaults
    let defaults = UserDefaults::new("group.com.hometier.app")
        .map_err(|e| format!("UserDefaults: {}", e))?;
    defaults.set_string("VPNConfig", &config_json);

    // 2. 触发 NE startVPNTunnel（通过 Swift 桥命令）
    let result = app_handle
        .invoke_key("plugin:ios-vpn|start_tunnel", serde_json::json!({}))
        .await;
    result.map_err(|e| format!("start_tunnel: {}", e))?;
    Ok(())
}
```

### 7.8 iOS 端到端时序

```
[前端] spacesStore.connect (iOS)
  ↓
[Rust] start_ios_vpn:
  - 生成 EasyTier TOML 配置
  - 序列化为 JSON
  - 写 App Group "VPNConfig"
  - 触发 NETunnelProviderManager.startVPNTunnel
  ↓
[iOS] 系统首次弹 VPN 授权框 → 用户批准 → 启动 HomeTierTunnelProvider
  ↓
[NE Swift] startTunnel(options, completion):
  - 读 App Group "VPNConfig"
  - init_logger（写到 App Group 容器文件）
  - run_network_instance(cfgStr)  // FFI → NE 进程内 easytier
  - register_stop_callback, register_running_info_callback
  - applyNetworkSettings：
      setTunnelNetworkSettings { ... completion ... }
      在 callback 内：
        fd = packetFlow KVC ?? tunnelFileDescriptor() (kern_control)
        setNonBlocking(fd)
        set_tun_fd(fd, &err)  // FFI → easytier NetworkInstance
  - completionHandler(nil)
  ↓
[NE easytier] 收到 fd → create_dev_for_mobile → TUN 就绪 → peer 通信
  ↓
[NE Swift] running_info callback 触发：
  - Darwin notify "com.hometier.app.status" → 主 app
  - 或 handleAppMessage "get_running_info" → 主 app 查询
  ↓
[主 app Tauri Rust] 监听 Darwin notify 或 NE handleAppMessage
  - 通过 tauri event "vpn:state" 转发到前端
  ↓
[前端] 收到 vpn:state { state: "connected" } → UI 更新
```

### 7.9 EasyTier-iOS 可直接复用清单

> **许可证前提**：EasyTier-iOS 是 **GPL-3.0**。本项目许可证需确认兼容（若本项目也是 GPL-3.0 即可；若不是，禁止直接 copy 代码）。

可复用文件：
- `PacketTunnelProvider.swift` —— 主体逻辑（含 fd KVC + kern_control 兜底）
- `TunnelHelper.swift` —— `tunnelFileDescriptor` (扫描 fd 0..1024 找 com.apple.net.utun_control)、`setNonBlocking`、`extractRustString`、`initRustLogger`
- `AddressHelper.swift` / `BuilderHelper.swift` —— CIDR/路由工具
- `kern_control.h` / `easytier_ios.h` —— FFI 头
- `Core/src/lib.rs` —— FFI 函数模板（修改为本项目 staticlib）

参考而非复制的部分（需要适配）：
- `Info.plist` —— 改 Bundle ID、MachServiceName
- `entitlements` —— 改 Group ID
- `EasyTierShared/` —— 本项目不需要（没有 Widget/共享组件）

---

## 8. 许可证与可复用性

### 8.1 EasyTier-iOS 许可证：GPL-3.0

**传染性**：
- 若本项目也是 GPL-3.0 → 可直接复制代码，合规
- 若本项目是 MIT/Apache-2.0/BSD → **不可**直接复制（会传染整个项目）
- 若本项目是 AGPL-3.0 → 兼容

### 8.2 复用建议
- **可复用**：参考算法/架构思路；FD 获取机制（公开技术）；FFI 函数签名设计
- **可复制（GPL 兼容时）**：上述 §7.9 清单中标注的文件
- **不可复制**：App Store 上架策略（合规问题）、SwiftUI UI 代码（本项目不用）

### 8.3 行动项
- [ ] **必须先确认本项目许可证**
- [ ] 若不兼容 → 仅借鉴思路，重写所有 Swift 代码（额外 ~500 行工作量）

---

## 9. 实施步骤（按提交拆分）

### 提交 1：Rust 侧 fd 注入抽象（跨平台骨架）
- 新增 `src-tauri/src/mobile/{mod,android,ios}.rs`
- 修改 `src-tauri/src/easytier/mod.rs`（mobile `set_tun_fd` impl）
- 修改 `src-tauri/src/space/manager.rs`（mobile connect 流程预留 VPN 异步）
- 修改 `src-tauri/src/lib.rs`（注册 `set_tun_fd` tauri 命令）
- **验证**：`cargo check --all-targets`

### 提交 2：Android Kotlin 层
- 新增 `scripts/android/HomeTierVpnService.kt`（~150 行）
- 新增 `scripts/android/MainActivityExt.kt`（~40 行）
- 扩展 `scripts/mobile-permissions.sh`（Android 注入）
- 修改 `tauri.conf.json` capabilities（`vpnservice:allow-*` 或自定义命令权限）
- 新增 tauri-plugin-android-vpn（精简版，~50 行 Rust 命令桥）
- **验证**：CI build-android 编译通过；本地无 Android SDK 跳过 native 验证

### 提交 3：前端连接流程（Android 端到端）
- 新增 `src/utils/api/vpn.ts`
- 修改 `src/stores/spacesStore.ts`（mobile 分支）
- 新增 `src/components/Space/VpnStatusBadge.tsx`（UI 状态）
- 修改 i18n（增加 VPN 授权/失败提示文案）
- **验证**：`tsc --noEmit` + `vite build`

### 提交 4：iOS staticlib crate
- 新增 `src-tauri/easytier-ios-staticlib/`（~200 行 Rust FFI）
- 修改 `src-tauri/Cargo.toml`（可选 workspace 配置）
- **验证**：`cargo check --target aarch64-apple-ios`（需 macOS + iOS SDK）

### 提交 5：iOS Swift NE 扩展（参考 EasyTier-iOS）
- 新增 `src-tauri/gen-scripts/ios/{PacketTunnelProvider,TunnelHelper,AddressHelper,BuilderHelper}.swift`
- 新增 `src-tauri/gen-scripts/ios/{kern_control,easytier_ios}.h`
- 新增 `src-tauri/gen-scripts/ios/{Info.plist,entitlements.entitlements}`
- **验证**：CI build-ios 编译（需 macOS runner）

### 提交 6：iOS Xcode 工程注入 + 主 app 集成
- 新增 `src-tauri/gen-scripts/ios/inject_ne_target.py`（~300 行）
- 修改 `src-tauri/build.rs`（iOS target 触发注入）
- 新增 `src-tauri/src-tauri/src/commands/ios_vpn.rs`（主 app 命令桥）
- 修改 `src-tauri/Info.ios.plist`（主 app NE entitlement + NSNetworkExtensionUsageDescription）
- 修改 release.yml build-ios 步骤（增加 NE 签名 + xcodebuild）
- **验证**：CI build-ios 完整流程

### 提交 7：iOS 端到端连通 + AppBrowser 降级
- 修改 `src/stores/spacesStore.ts`（iOS 分支）
- 修改 `src/components/App/ProxyFrame.tsx`（mobile 降级）
- 修改 `src/components/App/AppBrowser.tsx`（mobile 降级）
- i18n 增加 iOS VPN 状态文案
- **验证**：CI build-ios 全流程

### 提交 8：CI 完善与文档
- 修改 `.github/workflows/release.yml`（macOS runner 配置 + 签名 secrets 文档）
- 更新 `docs/` 下其他文档（开发文档、需求文档）交叉引用本文

---

## 10. CI 与构建

### 10.1 Android CI（保持现状，扩展 mobile-permissions.sh）

```yaml
# release.yml build-android job 已有步骤顺序（仅扩展 mobile-permissions.sh）
- name: Inject mobile permissions & VpnService
  run: bash scripts/mobile-permissions.sh

# 新增 secrets（无需新增）：
# （无）

# 新增能力：mobile-permissions.sh 已注入 HomeTierVpnService.kt + manifest
```

### 10.2 iOS CI（大幅扩展）

需求：
1. **macOS runner**（`macos-latest`，GitHub Actions 提供）
2. **Apple Developer 账号 secrets**：
   - `APPLE_TEAM_ID`
   - `APPLE_CERTIFICATE_P12`（base64）
   - `APPLE_CERTIFICATE_PASSWORD`
   - `APPLE_PROVISIONING_PROFILE_NE`（NE target 的 .mobileprovision，base64）
   - `APPLE_KEYCHAIN_PASSWORD`
3. **Build 步骤**：
```yaml
- name: Setup Apple code signing
  env:
    APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
  run: |
    KEYCHAIN_PATH=$RUNNER_TEMP/build.keychain-db
    security create-keychain -p "$KEYCHAIN_PASSWORD" $KEYCHAIN_PATH
    security set-keychain-settings -lut 21600 $KEYCHAIN_PATH
    security unlock-keychain -p "$KEYCHAIN_PASSWORD" $KEYCHAIN_PATH
    echo "$APPLE_CERTIFICATE_P12" | base64 --decode > /tmp/cert.p12
    security import /tmp/cert.p12 -k $KEYCHAIN_PATH -P "$APPLE_CERTIFICATE_PASSWORD" -A -T /usr/bin/codesign
    security list-keychain -d user -s $KEYCHAIN_PATH
    mkdir -p ~/Library/MobileDevice/Provisioning\ Profiles
    echo "$APPLE_PROVISIONING_PROFILE_NE" | base64 --decode > ~/Library/MobileDevice/Provisioning\ Profiles/ne.mobileprovision

- name: Inject NE target
  run: python3 src-tauri/gen-scripts/ios/inject_ne_target.py src-tauri/gen/apple/HomeTier.xcodeproj src-tauri/gen-scripts/ios

- name: Build iOS with NE
  run: |
    xcodebuild -project src-tauri/gen/apple/HomeTier.xcworkspace \
      -scheme HomeTier -configuration Release \
      -destination 'generic/platform=iOS' \
      CODE_SIGN_IDENTITY="Apple Development" \
      CODE_SIGN_STYLE=Manual \
      DEVELOPMENT_TEAM="$APPLE_TEAM_ID" \
      build
```

### 10.3 本地验证能力
- **Android**：本机无 Android SDK，全部依赖 CI（与现状一致）
- **iOS**：本机无 macOS，iOS 编译依赖 CI（与现状一致）
- **Rust**：本地 `docker exec rust-dev cargo check --all-targets --quiet` 验证桌面平台零回归
- **前端**：`./node_modules/.bin/tsc --noEmit && npm run build`

---

## 11. 测试策略

### 11.1 单元测试
- Rust `TunProvider` trait mock 实现测试
- FFI 函数（easytier-ios-staticlib）的 `cfg(test)` 集成测试（需 easytier 库 + tokio runtime）
- 前端 `spacesStore.connect` 流程的 vitest 单测（mock invoke）

### 11.2 集成测试
- **Android 真机/模拟器**：CI matrix 跑 `aarch64-linux-android` + `armv7-linux-androideabi`，手动测试 VPN 弹窗、连接、断开
- **iOS 真机/模拟器**：CI 仅能在 macos-latest runner 上跑模拟器（arm64-apple-ios-sim），手动测试 NE 配置申请、连接、断开

### 11.3 手动测试 checklist（Android）
- [ ] 首次连接弹出系统 VPN 授权对话框
- [ ] 用户拒绝授权 → 前端提示「请在系统设置中允许 VPN」
- [ ] 用户授权 → VpnService 启动 → 前台通知显示「准备连接…」
- [ ] fd 注入完成 → 前台通知显示「已连接」
- [ ] 设备上浏览器访问 easytier 虚拟网段内其他节点的 IP → 成功
- [ ] 断开连接 → VPN 自动卸载、前台通知消失
- [ ] 杀掉 app 进程 → VpnService 也清理（不残留通知/隧道）

### 11.4 手动测试 checklist（iOS）
- [ ] 首次连接 iOS 设置弹 VPN 配置申请对话框
- [ ] 用户批准 → iOS 设置显示 homeTier VPN 状态「已连接」
- [ ] 设备 Safari 访问虚拟网段 → 成功
- [ ] 断开 → VPN 状态「未连接」
- [ ] 杀掉 app → NE 进程独立保持/退出由系统决定（符合预期）

---

## 12. 风险与已知问题

### 12.1 高风险
1. **iOS NE 签名**：必须 Apple Developer 账号 + Network Extensions capability + provisioning profile。无账号则 iOS NE 走不通
2. **iOS Tauri + NE 工程集成**：Tauri 官方不支持 NE，需自维护 Xcode 工程注入脚本；Tauri 版本升级可能破坏
3. **fd 注入时序**：若 easytier 实例因其他原因已退出，set_tun_fd 会失败；需在前端处理重试逻辑
4. **iOS fd 来源合规**：KVC `packetFlow.value(forKeyPath: "socket.fileDescriptor")` 是私有 API 反射，可能被 App Store 审核拒绝；兜底 `tunnelFileDescriptor()` (kern_control) 是公开系统调用

### 12.2 中风险
5. **Android 14+ 前台服务类型**：必须声明 `foregroundServiceType="systemExempted"`，否则系统会杀进程
6. **iOS NE 沙箱**：extension 进程不能访问主 app 内存，所有状态必须通过 App Group 持久化
7. **Darwin notify 跨进程事件丢失**：状态同步需要重试/轮询兜底

### 12.3 低风险
8. **VpnService 通知**：用户可能误关闭前台通知 → 系统不会杀 VPN，但用户体验差
9. **Android 路由表冲突**：若 easytier 虚拟网段与其他 VPN（如公司 VPN）冲突，需提示用户先断开其他 VPN

---

## 13. 附录

### 13.1 关键文件路径索引
| 内容 | 路径 |
|------|------|
| 本文档 | `docs/mobile_vpn.md` |
| TunProvider trait | `src-tauri/src/mobile/mod.rs` |
| Android VpnService 模板 | `scripts/android/HomeTierVpnService.kt` |
| Android MainActivityExt | `scripts/android/MainActivityExt.kt` |
| iOS NE Swift 文件 | `src-tauri/gen-scripts/ios/*.swift` |
| iOS Xcode 注入脚本 | `src-tauri/gen-scripts/ios/inject_ne_target.py` |
| iOS staticlib crate | `src-tauri/easytier-ios-staticlib/` |
| 移动端权限注入脚本 | `scripts/mobile-permissions.sh` |
| 前端 VPN API | `src/utils/api/vpn.ts` |

### 13.2 关键 tauri 命令清单
| 命令 | 平台 | 签名 |
|------|------|------|
| `connect_space` | all | `connect_space(space_id: String)` |
| `disconnect_space` | all | `disconnect_space(space_id: String)` |
| `set_tun_fd` | mobile | `set_tun_fd(space_id: String, fd: i32)` |
| `prepare_vpn` | Android | `prepare_vpn() -> bool` |
| `start_vpn` | Android | `start_vpn(spaceId, ipv4Addr, routes, mtu)` |
| `stop_vpn` | Android | `stop_vpn()` |
| `start_ios_vpn` | iOS | `start_ios_vpn(spaceId, configJson)` |

### 13.3 关键事件清单
| 事件 | 方向 | payload |
|------|------|---------|
| `vpn:tun-ready` | Kotlin/Swift → Rust | `{ spaceId, fd }` |
| `vpn:status-changed` | Kotlin/Swift → Rust | `{ spaceId, status, error? }` |
| `vpn:state` | Rust → 前端 | `{ spaceId, state, error? }` |
| `vpn:state` (iOS Darwin notify) | NE → Rust | `{ spaceId, state, error? }` |

### 13.4 版本与依赖
- Tauri: ≥ 2.x
- easytier: ≥ 2.6.4（vendored at `src-tauri/resources/easytier_lib/easytier`）
- Android minSdk: 24（Android 8.0）
- Android targetSdk: 34（Android 14）
- iOS deployment target: 15.0
- Rust MSRV: 1.95
- Xcode: 15+

### 13.5 参考链接
- [EasyTier 主仓库](https://github.com/EasyTier/EasyTier)
- [EasyTier-iOS 仓库](https://github.com/EasyTier/EasyTier-iOS)
- [EasyTier `tauri-plugin-vpnservice` 参考](https://github.com/EasyTier/EasyTier/tree/main/tauri-plugin-vpnservice)
- [Apple NetworkExtension 文档](https://developer.apple.com/documentation/networkextension)
- [Android VpnService 文档](https://developer.android.com/reference/android/net/VpnService)
- [Apple NEPacketTunnelProvider 文档](https://developer.apple.com/documentation/networkextension/nepackettunnelprovider)
- [Tauri 移动端文档](https://tauri.app/v1/guides/distribution/mobile/)

---

**变更记录**
| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-08-18 | 1.0 | 初版（feat-serverization 阶段） |
