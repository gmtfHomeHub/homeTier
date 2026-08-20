# 许可证合规性检查报告

> 生成日期：2026-08-20
> 检查对象：homeTier 移动端 VPN 模块
> 参考：docs/mobile_vpn.md §8 许可证与可复用性

---

## 1. 本项目许可证现状

### 1.1 当前状态
- **根目录无 LICENSE 文件**
- **Cargo.toml 未声明 license 字段**
- **package.json 未声明 license 字段**

### 1.2 建议
**必须在发布前明确许可证**。建议选项：
- **GPL-3.0**：与 EasyTier/EasyTier-iOS 兼容，可直接复用代码
- **AGPL-3.0**：与 GPL-3.0 兼容，适合网络服务
- **MIT/Apache-2.0**：**不可**直接复用 EasyTier-iOS 代码（需重写）

---

## 2. 依赖许可证清单

### 2.1 核心依赖 (Rust)

| Crate | 许可证 | 兼容性 |
|-------|--------|--------|
| easytier (vendored) | GPL-3.0 | ⚠️ 需 GPL 兼容 |
| tauri | MIT | ✅ |
| tokio | MIT | ✅ |
| serde | MIT/Apache-2.0 | ✅ |
| rusqlite | MIT | ✅ |
| tracing | MIT | ✅ |
| 其他大部分 | MIT/Apache-2.0 | ✅ |

### 2.2 移动端特有依赖

| 组件 | 来源 | 许可证 | 状态 |
|------|------|--------|------|
| `HomeTierVpnService.kt` | 自写 (参考 EasyTier) | 项目许可证 | ✅ |
| `HomeTierVpnServicePlugin.kt` | 自写 | 项目许可证 | ✅ |
| `TauriEventBus` | 自写 | 项目许可证 | ✅ |
| `PacketTunnelProvider.swift` | **改编自 EasyTier-iOS** | **GPL-3.0** | ⚠️ |
| `TunnelHelper.swift` | **改编自 EasyTier-iOS** | **GPL-3.0** | ⚠️ |
| `BuilderHelper.swift` | **改编自 EasyTier-iOS** | **GPL-3.0** | ⚠️ |
| `AddressHelper.swift` | **改编自 EasyTier-iOS** | **GPL-3.0** | ⚠️ |
| `kern_control.h` | 公开系统头文件 | Public Domain | ✅ |
| `easytier_ios.h` | 自写 FFI 头文件 | 项目许可证 | ✅ |
| `inject_ne_target.py` | 自写 | 项目许可证 | ✅ |
| `easytier-ios-staticlib` | 自写 (链接 easytier) | **GPL-3.0** (传染) | ⚠️ |

---

## 3. GPL-3.0 传染性分析

### 3.1 传染路径

```
EasyTier (GPL-3.0) 
    │
    ├─→ easytier-ios-staticlib (静态链接) → **传染**
    │       │
    │       └─→ PacketTunnelProvider (动态加载 staticlib) → **传染**
    │
    └─→ 主程序 (通过 IPC/Unix socket 通信) → **边界情况**
```

### 3.2 关键判断点

| 场景 | 是否触发 GPL 传染 | 说明 |
|------|------------------|------|
| 静态链接 easytier 到主程序 | **是** | 主程序必须 GPL 兼容 |
| 动态加载 easytier-ios-staticlib (dlopen) | **是** | 视为合并作品 |
| NE Extension 独立进程 + IPC 通信 | **灰色地带** | FSF 认为插件机制仍传染 |
| 仅参考算法/架构，重写代码 | **否** | 思想不受版权保护 |

### 3.3 结论

**当前实现**：
- `easytier-ios-staticlib` 静态链接 `easytier` (GPL-3.0) → **必须 GPL-3.0 兼容**
- `PacketTunnelProvider.swift` 等 Swift 文件**直接改编自 EasyTier-iOS (GPL-3.0)** → 必须遵守 GPL-3.0

**因此：本项目移动端 VPN 模块实际上已处于 GPL-3.0 管辖下**。

---

## 4. 合规行动方案

### 方案 A：采用 GPL-3.0 (推荐，最省事)

```toml
# Cargo.toml
license = "GPL-3.0-or-later"

# package.json
"license": "GPL-3.0-or-later"
```

**行动项**：
1. [ ] 在根目录添加 `LICENSE` 文件 (GPL-3.0 全文)
2. [ ] 在 `src-tauri/Cargo.toml` 添加 `license = "GPL-3.0-or-later"`
3. [ ] 在 `package.json` 添加 `"license": "GPL-3.0-or-later"`
4. [ ] 在所有源文件头部添加 GPL 版权声明
5. [ ] 提供源码获取途径 (GitHub 仓库即满足)

**优点**：可直接使用所有现有代码，无需重写
**缺点**：商业使用受限，衍生作品必须开源

---

### 方案 B：重写为 MIT/Apache-2.0 兼容 (工作量大)

**需重写的文件**：
1. `src-tauri/gen-scripts/ios/PacketTunnelProvider.swift` (~1300 行)
2. `src-tauri/gen-scripts/ios/TunnelHelper.swift` (~500 行)
3. `src-tauri/gen-scripts/ios/BuilderHelper.swift` (~500 行)
4. `src-tauri/gen-scripts/ios/AddressHelper.swift` (~300 行)
5. `src-tauri/easytier-ios-staticlib/` 整个 crate (需替换为非 GPL 依赖)

**替代方案**：
- 使用 `networkextension` crate (MIT) 替代直接调用
- 参考 [WireGuard iOS](https://github.com/WireGuard/wireguard-apple) (Apache-2.0) 实现
- 完全自行实现 NEPacketTunnelProvider 逻辑

**预估工作量**：~2-3 人周

---

### 方案 C：架构隔离 (复杂)

将 GPL 组件完全隔离到独立进程，仅通过标准 IPC 通信：
- NE Extension 独立签名、独立分发
- 主程序通过 `NETunnelProviderManager` 系统 API 通信
- **风险**：FSF 立场认为插件/扩展机制仍构成合并作品

---

## 5. 建议决策

### 立即行动 (发布前必做)
1. [ ] **确定项目许可证** - 团队决策会议
2. [ ] **添加 LICENSE 文件** - 根目录
3. [ ] **更新 Cargo.toml / package.json** - 声明 license 字段

### 短期 (v0.2.0 前)
- 如果选方案 A：完成 GPL 合规文档，添加版权头
- 如果选方案 B：启动 Swift 层重写任务

### 长期
- 考虑将移动端 VPN 抽离为独立 GPL 仓库
- 主程序保持 MIT/Apache-2.0，通过标准接口集成

---

## 6. 文件级版权头模板

### Rust 文件
```rust
// Copyright (C) 2024 homeTier Authors
//
// This file is part of homeTier.
//
// homeTier is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// homeTier is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with homeTier.  If not, see <https://www.gnu.org/licenses/>.
```

### Swift/Kotlin 文件
```swift
/*
 * Copyright (C) 2024 homeTier Authors
 *
 * This file is part of homeTier.
 *
 * homeTier is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * homeTier is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with homeTier.  If not, see <https://www.gnu.org/licenses/>.
 */
```

---

## 7. 签名确认

| 角色 | 决策 | 日期 | 签名 |
|------|------|------|------|
| 法务/合规 |  |  |  |
| 技术负责人 |  |  |  |
| 项目负责人 |  |  |  |

---

*本报告基于 docs/mobile_vpn.md §8 许可证与可复用性生成，需团队审议决策。*