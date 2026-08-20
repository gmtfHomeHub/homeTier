# homeTier 移动端 VPN 手动测试清单

> 版本：1.0
> 适用：Android (VpnService) / iOS (NetworkExtension)
> 来源：docs/mobile_vpn.md §11 测试策略

---

## 1. Android 真机/模拟器测试

### 1.1 基础连接流程

| # | 测试步骤 | 预期结果 | 实际结果 | 备注 |
|---|---------|---------|---------|------|
| A1 | 首次打开应用，点击「连接」某空间 | 系统弹出 VPN 授权对话框 |  |  |
| A2 | 点击「拒绝」授权 | 前端提示「VPN 授权被拒绝，请在系统设置中允许 VPN 权限」 |  |  |
| A3 | 点击「允许」授权 | VpnService 启动，前台通知显示「准备连接…」 |  |  |
| A4 | 等待 fd 注入完成 | 前台通知显示「已连接」，空间状态变为「已连接」 |  |  |
| A5 | 在浏览器访问 easytier 虚拟网段内其他节点 IP | 访问成功 |  |  |
| A6 | 点击「断开」 | VPN 自动卸载，前台通知消失，空间状态变为「已断开」 |  |  |
| A7 | 杀掉 app 进程 | VpnService 也清理，不残留通知/隧道 |  |  |

### 1.2 异常场景

| # | 测试步骤 | 预期结果 | 实际结果 | 备注 |
|---|---------|---------|---------|------|
| A8 | VPN 建立过程中杀掉 app | 后台服务正确停止，无资源泄漏 |  |  |
| A9 | 切换到另一个空间 | 原空间自动断开，新空间连接 |  | 空间互斥 |
| A10 | 网络切换 (WiFi ↔ 4G/5G) | VPN 自动重连或保持连接 |  |  |
| A11 | 系统设置中手动关闭 VPN | 前端检测到状态变化，更新为「已断开」 |  |  |
| A12 | 同一设备多次连接/断开 | 无内存泄漏，无 fd 泄漏 |  |  |

### 1.3 Android 版本兼容性

| Android 版本 | API Level | 测试结果 | 备注 |
|-------------|-----------|---------|------|
| Android 8.0 | 26 |  | 最低支持版本 |
| Android 10 | 29 |  |  |
| Android 12 | 31 |  |  |
| Android 13 | 33 |  |  |
| Android 14 | 34 |  | 需 FOREGROUND_SERVICE_SYSTEM_EXEMPTED |

---

## 2. iOS 真机/模拟器测试

### 2.1 基础连接流程

| # | 测试步骤 | 预期结果 | 实际结果 | 备注 |
|---|---------|---------|---------|------|
| I1 | 首次点击「连接」某空间 | iOS 设置弹出 VPN 配置申请对话框 |  |  |
| I2 | 点击「不允许」 | 前端提示授权被拒绝 |  |  |
| I3 | 点击「允许」 | NE 扩展启动，iOS 设置显示 homeTier VPN 状态「已连接」 |  |  |
| I4 | 在 Safari 访问虚拟网段 | 访问成功 |  |  |
| I5 | 点击「断开」 | VPN 状态变为「未连接」 |  |  |
| I6 | 杀掉 app | NE 进程独立保持/退出由系统决定 |  | 符合预期 |

### 2.2 异常场景

| # | 测试步骤 | 预期结果 | 实际结果 | 备注 |
|---|---------|---------|---------|------|
| I7 | VPN 建立过程中杀掉 app | NE 扩展正确清理 |  |  |
| I8 | 切换到另一个空间 | 原空间自动断开，新空间连接 |  | 空间互斥 |
| I9 | 网络切换 (WiFi ↔ 蜂窝) | VPN 自动重连 |  |  |
| I10 | 系统设置中手动关闭 VPN | 前端检测到状态变化 |  |  |
| I11 | 设备重启后 | VPN 不自动连接（需用户手动点击） |  | 无开机自启 |

### 2.3 iOS 版本兼容性

| iOS 版本 | 测试结果 | 备注 |
|---------|---------|------|
| iOS 15.0 |  | 最低支持版本 |
| iOS 16.x |  |  |
| iOS 17.x |  |  |

---

## 3. 跨平台一致性测试

| # | 测试场景 | Android | iOS | 备注 |
|---|---------|---------|-----|------|
| C1 | 同一空间在 Android 和 iOS 同时连接 |  |  | 仅最后连接的生效（空间互斥） |
| C2 | 桌面端连接后，移动端尝试连接同一空间 |  |  | 移动端应提示冲突或自动断开桌面端 |
| C3 | 虚拟 IP 分配一致性 |  |  | 相同配置应分配相同虚拟 IP 段 |
| C4 | 路由表一致性 |  |  | 仅虚拟网段路由，不劫持默认路由 |

---

## 4. 性能指标

| 指标 | 目标 (P95) | Android 实测 | iOS 实测 | 备注 |
|------|-----------|-------------|---------|------|
| VPN 授权到 fd 注入完成 | ≤ 3s |  |  |  |
| 首包延迟 (同一局域网) | < 50ms |  |  |  |
| 吞吐量 (同一局域网) | > 50 Mbps |  |  |  |
| 内存占用 (VPN 运行时) | < 50 MB |  |  |  |
| 电量影响 (24h 待机) | < 5% |  |  |  |

---

## 5. 日志关键点验证

### 5.1 Android 端关键日志

```
# VpnService 启动
D/HomeTierVpn: onStartCommand: Bundle[{IPV4_ADDR=10.144.144.1/24, ROUTES=[10.144.144.0/24], SPACE_ID=...}]

# fd 发送
D/TauriEventBus: Emitted event: vpn:tun-ready, payload: {"spaceId":"...","fd":42}

# Rust 接收并注入
I/EasyTierManager: TUN fd 42 injected for instance ...

# 前端收到状态
vpn:state {spaceId: "...", state: "connected"}
```

### 5.2 iOS 端关键日志

```
# PacketTunnelProvider 启动
I/com.hometier.app.tunnel: startTunnel called

# 读取配置
I/com.hometier.app.tunnel: Starting network instance with config: {...}

# 网络设置应用
I/com.hometier.app.tunnel: Tunnel network settings applied successfully

# fd 获取
I/com.hometier.app.tunnel: Got TUN fd from packetFlow KVC: 5
I/com.hometier.app.tunnel: TUN fd 5 set successfully

# 跨进程通知
Darwin notify: com.hometier.app.vpn.running_info
```

---

## 6. 回归测试矩阵

| 变更类型 | 必须重跑的测试用例 |
|---------|------------------|
| VPN 权限/授权流程 | A1, A2, A3, I1, I2, I3 |
| fd 注入逻辑 | A4, A5, I4 |
| 断开/清理逻辑 | A6, A7, I5, I6 |
| 空间切换 | A9, I8, C1, C2 |
| 网络切换 | A10, I9 |
| 版本升级 | 全量回归 |

---

## 7. 问题记录模板

| 字段 | 内容 |
|------|------|
| 问题编号 | VPNT-XXXX |
| 平台 | Android / iOS |
| 复现步骤 | 1. ... 2. ... 3. ... |
| 预期结果 | ... |
| 实际结果 | ... |
| 日志片段 | `...` |
| 严重程度 | P0/P1/P2 |
| 修复版本 | v0.x.x |

---

## 8. 签名确认

| 角色 | 姓名 | 日期 | 签名 |
|------|------|------|------|
| 测试工程师 |  |  |  |
| 开发负责人 |  |  |  |
| 发布经理 |  |  |  |

---

## 附录：常用调试命令

### Android
```bash
# 查看 VpnService 日志
adb logcat -s HomeTierVpn TauriEventBus

# 查看 VPN 接口
adb shell ip addr show tun0

# 查看路由表
adb shell ip route show

# 强制停止 VpnService
adb shell am force-stop com.hometier.app
```

### iOS
```bash
# 查看 NE 扩展日志 (需连接 Xcode)
# Console.app -> 选择设备 -> 过滤 subsystem: com.hometier.app.tunnel

# 查看 Darwin 通知
# 无直接命令，需在主 app 中监听
```

---

*文档维护：移动端 VPN 模块开发团队*