# homeTier

> 基于 EasyTier 的跨平台虚拟局域网应用 / A cross-platform virtual LAN application powered by EasyTier

[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-24C8D8)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18-blue)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.5%20strict-3178C6)](https://www.typescriptlang.org)
[![EasyTier](https://img.shields.io/badge/EasyTier-2.6.4-green)](https://github.com/EasyTier/EasyTier)

---

## 目录 / Table of Contents

- [项目简介 / Introduction](#项目简介--introduction)
- [技术栈 / Tech Stack](#技术栈--tech-stack)
- [架构总览 / Architecture Overview](#架构总览--architecture-overview)
- [快速开始 / Quick Start](#快速开始--quick-start)
- [服务器模式 / Server Mode](#服务器模式--server-mode)
- [配置与数据 / Configuration & Data](#配置与数据--configuration--data)
- [前后端通信契约 / Frontend–Backend Contract](#前后端通信契约--frontendbackend-contract)
- [目录结构 / Directory Structure](#目录结构--directory-structure)
- [功能特性 / Feature Highlights](#功能特性--feature-highlights)
- [文档导航 / Documentation](#文档导航--documentation)

---

## 项目简介 / Introduction

**homeTier** 是一款跨平台（Windows / macOS / Linux，含 Android / iOS 移动端桩代码）的虚拟局域网应用。它基于 [EasyTier](https://github.com/EasyTier/EasyTier) 组网内核，为多个设备构建加密的 P2P 虚拟网络（空间），并在其上提供聊天、语音、屏幕共享、文件传输、局域网应用访问与分布式配置存储等能力。

**homeTier** is a cross-platform (Windows / macOS / Linux, with Android/iOS stubs) virtual LAN application built on the [EasyTier](https://github.com/EasyTier/EasyTier) networking kernel. It establishes encrypted P2P virtual networks ("spaces") among devices, on top of which it provides chat, voice, screen sharing, file transfer, LAN app browsing and distributed config storage.

**三大运行模式 / Three runtime modes:**

| 模式 Mode | 入口 Entry | 说明 Description |
|---|---|---|
| 桌面 GUI Desktop GUI | `homeTier`（默认） | Tauri 原生窗口 + WebView 前端，daemon 以子进程运行（macOS 经 osascript 提权） |
| 守护进程 Daemon | `homeTier --daemon` | 无头后台进程，提供 TCP IPC 服务，管理 EasyTier 网络 |
| 服务器模式 Server | `homeTier --server` | 单进程 axum HTTP 服务器，提供 Web 管理界面 + REST/WS API，内嵌 daemon |

---

## 技术栈 / Tech Stack

### 后端 / Backend

| 组件 | 选型 |
|---|---|
| 语言 | Rust 2021 edition（MSRV 1.75） |
| 桌面框架 | Tauri 2.x（插件：shell / process / clipboard / global-shortcut / os / notification / dialog / single-instance） |
| 异步运行时 | Tokio（full） |
| 组网内核 | 内置 EasyTier 2.6.4（`src-tauri/resources/easytier_lib/easytier`），features: `wireguard` `websocket` `tun` `socks5` `kcp` `quic` `zstd` |
| HTTP 服务器 | axum 0.8（服务器模式 REST API + WS） |
| HTTP 代理 | hyper 1.x + http-body-util（内嵌 iframe 代理） |
| 数据库 | SQLite（rusqlite bundled，自动迁移） |
| WebRTC | webrtc 0.11（语音 / 屏幕共享） |
| 安全 | aes-gcm（AES-256-GCM）、pbkdf2（210k 次迭代）、sha2、hmac、zstd（压缩） |
| 日志 | 自定义日志系统：内存环形缓冲 / 文件轮转 / JSON stdout / syslog / 转发 daemon 五类后端 |

### 前端 / Frontend

| 组件 | 选型 |
|---|---|
| 框架 | React 18 + TypeScript 5.5（strict） |
| 构建 | Vite 5（dev 端口 1420，strictPort） |
| 状态管理 | Zustand 4（9 个 store） |
| 样式 | Tailwind CSS 3 + Radix Themes 3 |
| 路由 | React Router 6 |
| 国际化 | react-i18next（zh / zh-TW / en，默认 zh） |
| 包管理 | pnpm 9+ |

---

## 架构总览 / Architecture Overview

### 进程拓扑 / Process Topology

```
┌──────────────────────────┐      ┌──────────────────────────┐
│  GUI / Server 进程        │      │  服务器模式（单进程）       │
│  (Tauri / axum)          │      │  run_server()            │
│        │                 │      │  内嵌 daemon (tokio task) │
│        │ TCP IPC :15889  │      │        │                 │
│        ▼                 │      │        │                 │
│  daemon 子进程            │      │        ▼                 │
│  (可提权 root/管理员)      │      │  daemon (同进程)          │
│        │ gRPC/TCP :15888 │      │        │ gRPC/TCP :15888  │
│        ▼                 │      │        ▼                 │
│  easytier-core           │      │  easytier-core           │
│  （TUN 虚拟网卡）          │      │  （TUN 虚拟网卡）          │
└──────────────────────────┘      └──────────────────────────┘
```

- **空间互斥**：同一时间只运行一个 EasyTier 网络实例，`SpaceManager::connect` 会先断开当前连接。
- **前后端解耦**：GUI 与 daemon 通过 `127.0.0.1:15889` 长度前缀 JSON IPC 通信；服务器模式在进程内嵌 daemon，复用同一套 IPC 协议。

### 核心模块 / Core Modules

| 模块 Module | 职责 Responsibility |
|---|---|
| `app/` | Tauri 生命周期粘合：`setup`（初始化 DB / daemon / 代理 / 托盘）、`exit` 清理、窗口显隐、提权标记 |
| `commands/` | 80 个 `#[tauri::command]`，18 个模块（space / network / chat / file / voice / screen / proxy / config / easytier / daemon / log / tray / signal 等） |
| `daemon/` | 无头守护进程：TCP IPC 服务器、easytier-core 生命周期管理、S5 GUI 看门狗、优雅退出 |
| `space/` | 空间编排中枢：创建/加入/离开/连接/断开、peer 发现、聊天/语音/屏幕/文件服务器生命周期、加密分享链接 |
| `easytier/` | EasyTier 管理器：RPC 驱动 `easytier-core`（桌面）或进程内 launcher（移动端）、TOML 配置生成、二进制下载/升级 |
| `chat/` | P2P 聊天：每空间 HTTP 服务器 + 向 peer 广播，消息 HMAC 签名校验 |
| `voice/` `screen/` | WebRTC 语音/屏幕共享引擎 + 信令服务器（端口 18100+ / 18200） |
| `file/` | P2P 文件传输：zstd 压缩 + 可选 AES 加密、流式收发带进度、HTTP 文件服务器（19000 + space_id % 1000） |
| `proxy/` | 内嵌 HTTP 代理（127.0.0.1 随机端口）：CORS / iframe 绕过 / HTTPS 隧道 / URL 重写 / WebSocket 隧道 / `__proxy__` 本地 HTTP 代理 |
| `server/` | 服务器模式：axum 路由（`/api/cmd/*` 约 60 条）、WebSocket、静态资源（内嵌 dist）、Cookie 鉴权、TLS、事件总线 |
| `config_store/` | P2P 分布式配置存储（TCP 9877）：版本化文件 + 校验和 + 去重写队列 |
| `db/` | SQLite 持久化（8 张表），启动自动迁移 |
| `log/` | 统一日志系统与 `log_info!` / `log_warn!` / `log_error!` / `log_debug!` 宏 |
| `crypto/` | AES-256-GCM + PBKDF2-HMAC-SHA256（210k 迭代）、SHA-256、HMAC 签名 |
| `platform/` | `PlatformAdapter` 平台抽象（配置/日志目录）、机器标识读取 |

### 三种模式差异 / Mode Comparison

| 维度 | 桌面 GUI | 守护进程 daemon | 服务器 Server |
|---|---|---|---|
| 前端 | Tauri WebView | 无 | 任意浏览器（axum 托管 `dist/`） |
| daemon 位置 | 独立子进程（macOS 提权） | 自身 | 同进程内嵌 |
| 命令接口 | `invoke()` 80 个 command | TCP IPC | REST `/api/cmd/*` + WS |
| 实时事件 | Tauri `listen("new_message")` | — | WS `/ws/events` + `/ws/signal/{spaceId}` |
| 日志 | 内存 + 转发 | IPC WriteLog | JSON stdout + 文件 + syslog |
| 配置 | `{app_data_dir}/homeTier.conf` | `{data_dir}/homeTier.conf` | `{server-dir}/homeTier.conf` + `server.conf` |

---

## 快速开始 / Quick Start

### 环境要求 / Prerequisites

| 工具 | 版本 |
|---|---|
| Rust | 1.75+（vendored EasyTier 需 1.95 编译） |
| Node.js | 18+ |
| pnpm | 9+ |
| Tauri CLI | 2.x（`cargo install tauri-cli --version "^2"`） |

平台编译工具链（Tauri 2 官方要求）：Windows 需 VS Build Tools（含 C++ 桌面工作负载）；macOS 需 Xcode；Linux 需 `libwebkit2gtk-4.1-dev`、`libappindicator3-dev` 等。

### 开发 / Development

```bash
pnpm install                     # 安装前端依赖
pnpm tauri dev                   # 启动 Tauri 开发（Vite:1420 + Rust）
```

仅调试前端（无原生窗口）：

```bash
pnpm dev                         # Vite dev server on http://localhost:1420
```

### 构建 / Build

```bash
pnpm tauri build                 # 生产构建（tsc --noEmit && vite build + Rust release + 平台安装包）
```

### 后端类型检查 / Backend Type Check

```bash
cd src-tauri && cargo check      # 需要本机 cc linker（Windows/macOS 自带）
```

> **Linux 宿主无 cc linker 时**：使用项目 Docker 开发容器验证编译：
> ```bash
> docker exec -w /workspace/homeTier/src-tauri rust-dev cargo check --bin homeTier
> ```

---

## 服务器模式 / Server Mode

服务器模式将 homeTier 变成单进程 Web 服务：一个进程同时提供 Web 管理界面、REST/WS API 与内嵌 daemon（默认监听 `0.0.0.0:9339`）。

### CLI 参数 / CLI Flags

| 参数 | 默认值 | 说明 |
|---|---|---|
| `--server` | — | 启用服务器模式 |
| `--server-bind` | `0.0.0.0` | 监听地址 |
| `--server-port` | `9339` | 监听端口 |
| `--server-dir` | `./homeTier-data` | 数据目录（DB、server.conf、easytier 配置） |
| `--server-resource-dir` | 内置 | 资源目录（easytier-core 兜底二进制） |
| `--server-static-dir` | 内嵌 dist | 前端静态资源目录（未提供时用编译期内嵌资源） |

### server.conf（数据目录内生成 / Generated in data dir）

| 键 Key | 默认值 | 说明 |
|---|---|---|
| `SERVER_BIND` | `0.0.0.0` | 监听地址 |
| `SERVER_PORT` | `9339` | HTTP 端口 |
| `SERVER_STATIC_DIR` | `./dist` | 静态资源目录 |
| `SERVER_TLS` | `false` | 启用 TLS（需同时配置证书） |
| `SERVER_TLS_CERT` / `SERVER_TLS_KEY` | 空 | PEM 证书 / 私钥路径 |
| `SERVER_AUTH_SECRET` | 自动生成 | Cookie 鉴权 HMAC 密钥（32 字节 hex） |
| `SERVER_CORS_ORIGIN` | `*` | 允许的 CORS 来源（`*` 时禁用凭据） |
| `SERVER_PROXY_PREFIX` | `/proxy` | 内嵌代理前缀 |

### Docker 部署 / Docker Deployment

项目根目录提供 `Dockerfile`（构建阶段 Node 22 + Rust，运行阶段 `debian:bookworm-slim`）：

```bash
docker build -t hometier .
docker run -d --name hometier --restart unless-stopped \
  -p 9339:9339 \
  -v hometier-data:/home/hometier/.local/share/homeTier \
  hometier
```

> 镜像默认 `EXPOSE 15888 15889 9339`，非 root 用户 `hometier` 运行，数据目录 `$HOME/.local/share/homeTier`。

### systemd 部署 / systemd Deployment

参见 `deploy/hometier-server.service`：

```bash
install -d /opt/homeTier
cp -r dist /opt/homeTier/dist
cp homeTier.conf.example /opt/homeTier/homeTier.conf
install -m 755 src-tauri/target/release/homeTier /opt/homeTier/homeTier
cp deploy/hometier-server.service /etc/systemd/system/
systemctl daemon-reload && systemctl enable --now hometier-server
```

---

## 配置与数据 / Configuration & Data

### homeTier.conf

应用配置为 `.env` 风格 `KEY=VALUE` 文件，支持热加载（2s mtime 轮询，修改即生效）。优先级：**运行时配置 > 模板默认值（`homeTier.conf.example`）> 内置默认值**。模板位于项目根 `homeTier.conf.example`，首次启动复制到数据目录。

| 键 Key | 默认值 | 说明 |
|---|---|---|
| `DAEMON_IPC_PORT` | `15889` | daemon IPC 端口 |
| `EASYTIER_RPC_PORT` | `15888` | easytier-core RPC 端口 |
| `FILE_SERVER_PORT_BASE` | `19000` | 文件服务器端口基数（实际 = 基数 + space_id % 1000） |
| `DEFAULT_SPACE_IP` | `10.144.144.10` | 新建空间默认虚拟 IPv4 |
| `GITHUB_API` | GitHub Releases API | EasyTier 版本检查 |
| `GITHUB_MIRROR` | `https://ghproxy.top` | 下载镜像前缀（留空直连 GitHub） |
| `RELAY_NETWORK_PREFIX` | `homeTier_` | 中继网络前缀 |
| `LOG_ENABLED` | `1` | 日志开关 |

### 数据库 / Database

SQLite 文件位于 `{app_data_dir}/homeTier.db`（服务器模式为 `{server-dir}/homeTier.db`），启动自动迁移（`src-tauri/src/db/migrations.rs`），共 8 张表：

| 表 | 说明 |
|---|---|
| `users` | 本机用户（机器标识） |
| `spaces` | 空间（含 `network_secret`、`config_json`） |
| `members` | 成员（虚拟 IP、在线状态、是否 owner） |
| `messages` | 聊天消息（含发送状态） |
| `files` | 文件记录 |
| `settings` | 键值设置 |
| `space_apps` | 空间内应用（iframe 浏览器用） |
| `acl_rules` / `port_forward_rules` | ACL 规则 / 端口转发规则 |

### 端口约定 / Port Conventions

| 端口 | 用途 |
|---|---|
| `15889` | daemon TCP IPC（可配置） |
| `15888` | easytier-core RPC（可配置） |
| `19000 + space_id % 1000` | 文件传输 HTTP 服务器 |
| `18100 + space_id % 100` | 语音信令服务器 |
| `18200` | 屏幕共享信令服务器 |
| `9877` | 分布式配置存储 TCP（P2P） |
| `9339` | 服务器模式 HTTP（可配置） |

### 分享链接 / Share Links

格式：`homeTier://join?v=3&d={base64url}`。载荷流程：ShareInfo 序列化 → **zstd 压缩（level 3）** → **AES-256-GCM 加密**（密钥 = SHA-256("homeTier-share-link-v2")，版本化便于轮换）→ base64url。聊天消息使用空间 `network_secret` 做 HMAC-SHA256 签名校验；密码保护文件使用 PBKDF2 派生密钥加密。

---

## 前后端通信契约 / Frontend–Backend Contract

### Tauri 命令（桌面）/ Tauri Commands

- **80 个** `#[tauri::command]`，分布于 `src-tauri/src/commands/` 18 个模块，在 `src-tauri/src/lib.rs` 注册。
- 前端通过 `src/utils/api.ts` 统一封装：运行时检测 `__TAURI_INTERNALS__` 自动选择 **Tauri `invoke()`**（`utils/api/tauri.ts`）或 **REST/WS**（`utils/api/web.ts`）实现，业务代码无感。
- 主要命令域：space（14）、config_store（8）、file（6）、util/app（5+5）、voice/screen（4+4）、proxy（4）、ACL/端口转发（4+4）、easytier（4）、daemon/config（4+4）、network/log（3+3）、chat（2）、tray/signal（1+1）。

### 服务器模式 REST + WS / Server Mode API

- REST：`/api/cmd/*`（ping / space / chat / network / log / config / file / proxy / easytier / config-store 等约 60 条路由），JSON + cookie 鉴权。
- WebSocket：`/api/cmd/ws/events`（全局事件流）、`/api/cmd/ws/signal/{spaceId}`（WebRTC 信令转发）。
- 事件类型（`server/event.rs`）：SpaceCreated/Deleted/Updated、MemberJoined/Left、MessageSent、FileShared、ScreenShareStarted/Stopped、VoiceCallStarted/Stopped、PeerConnected/Disconnected、ConfigChanged、SystemLog。

### 前端事件（桌面）/ Frontend Events

`new_message`（聊天/信令）、`tray-navigate`（托盘导航）、`daemon-ready`（daemon 就绪）、`easytier-download-progress`（升级进度）、`config:changed`（配置热更新）。

### WebRTC 信令（关键设计）/ WebRTC Signaling

语音/屏幕共享的**信令控制面复用聊天消息通道**：`msg_type="signal"` 携带 `SignalEnvelope {kind, type, from, to, data}`，经 `realtime.ts` 分发到 `signal.ts`，再路由至 `voice.ts` / `screen.ts`（浏览器端全网格 WebRTC，无后端媒体面）。确定性 offerer：虚拟 IP 字典序较小者为 offerer。

### 类型同步 / Type Sync

| 后端 | 前端 | 说明 |
|---|---|---|
| `src-tauri/src/types.rs` | `src/types/index.ts` | Space / Member / Message / FileInfo 等并行类型，手动同步 |
| `easytier/config.rs` 的 `NetworkConfig` | `src/types/network.ts` | 网络配置镜像（含 `DEFAULT_NETWORK_CONFIG()`） |

---

## 目录结构 / Directory Structure

```
homeTier/
├── src/                        # 前端 React/TS
│   ├── components/             # 按功能域组织的 UI（Layout/Space/Chat/Voice/...）
│   ├── stores/                 # Zustand（9 个 store：space/settings/file/chat/voice/screen/appTabs/...）
│   ├── services/               # realtime / signal / voice / screen / shortcuts
│   ├── utils/                  # api.ts（双模式入口）+ api/{tauri,web,core}.ts + 工具
│   ├── i18n/                   # locales（zh / zh-TW / en）
│   ├── types/                  # index.ts（领域模型）+ network.ts（NetworkConfig）
│   └── hooks/  enum/  styles/
├── src-tauri/                  # 后端 Rust
│   ├── src/                    # 见"核心模块"表
│   ├── resources/easytier_lib/easytier   # 内置 EasyTier 2.6.4（vendored）
│   ├── resources/bin/          # easytier-core 兜底二进制
│   ├── tauri.conf.json         # Tauri 配置（identifier: com.hometier.app, v0.1.0）
│   └── Cargo.toml
├── docs/                       # 设计文档（需求/设计/开发/服务器化改造等）
├── deploy/hometier-server.service  # systemd 部署单元
├── Dockerfile                  # 服务器模式容器镜像
├── homeTier.conf.example       # 配置模板
└── package.json / pnpm-lock.yaml
```

### 前端路由 / Frontend Routes

| 路由 | 页面 |
|---|---|
| `/` | 空间列表（连接/分享/配置/删除） |
| `/space/:id` | 空间主页（网络统计 + 应用启动器） |
| `/space/:id/chat` `/voice` `/screen` `/files` `/logs` | 聊天 / 语音 / 屏幕共享 / 文件 / 日志 |
| `/space/:id/app/:appId` | 应用 iframe 标签深链 |
| `/settings` | 设置（基础 / EasyTier / 配置 / 日志 四个页签） |
| `*` | 404 |

---

## 功能特性 / Feature Highlights

- **空间互斥连接**：单 EasyTier 实例，连接新空间自动断开旧空间，托盘菜单随语言/状态同步。
- **P2P 聊天**：消息 HMAC 签名校验，乐观更新 + 去重，虚拟化消息列表。
- **WebRTC 语音**：全网格直连，RMS 语音活动检测（150ms 采样、1.2s 静音自动闭麦）、每 peer 音量条、全局快捷键 `Ctrl+M`/`Ctrl+T`（带 OSD）。
- **屏幕共享**：邀请制 ACL、质量切换（smooth/standard/hd，maxBitrate 控制）。
- **文件传输**：zstd 压缩 + 可选密码加密，流式收发带进度，断点续传进度恢复。
- **应用浏览器**：内嵌 iframe 访问空间内任意 HTTP 应用（最多 10 个 LRU 标签页），经内嵌代理（hyper）绕过 CSP/X-Frame-Options，注入 fetch/XHR/WebSocket shim 统一重写 URL；桌面/移动视口缩放。
- **分布式配置存储**：TCP 9877 P2P 版本化配置同步，防版本回滚。
- **安全**：分享链接 AES-256-GCM 加密 + zstd；聊天 HMAC 校验；文件 PBKDF2(210k) 派生密钥；机器标识防重放。
- **国际化**：中/繁/英三语，托盘菜单随语言热同步。

---

## 文档导航 / Documentation

| 文档 | 说明 |
|---|---|
| [需求文档](docs/需求文档.md) | 产品需求 |
| [设计文档](docs/设计文档.md) | 系统设计 |
| [开发文档](docs/开发文档.md) | 开发环境搭建与开发指南 |
| [服务器化改造](docs/服务器化改造.md) | 服务器模式设计 |
| [分布式配置文件存储服务设计文档](docs/分布式配置文件存储服务设计文档.md) | 配置存储服务设计 |
| [接入三方应用设计文档](docs/接入三方应用设计文档.md) | 三方应用接入方案 |

---

## License

本项目基于内部开发使用，相关依赖遵循其各自的开源许可（EasyTier: Apache-2.0）。
This project is for internal development; dependencies are subject to their respective licenses (EasyTier: Apache-2.0).
