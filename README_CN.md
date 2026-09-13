# homeTier

> 基于 EasyTier 的跨平台虚拟局域网应用

[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-24C8D8)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18-blue)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.5%20strict-3178C6)](https://www.typescriptlang.org)
[![EasyTier](https://img.shields.io/badge/EasyTier-2.6.4-green)](https://github.com/EasyTier/EasyTier)

**中文** | **[English](README.md)**

---

## 目录

- [项目简介](#项目简介)
- [技术栈](#技术栈)
- [架构总览](#架构总览)
- [快速开始](#快速开始)
- [服务器模式](#服务器模式)
- [配置与数据](#配置与数据)
- [前后端通信契约](#前后端通信契约)
- [目录结构](#目录结构)
- [功能特性](#功能特性)
- [平台支持与已知限制](#平台支持与已知限制)
- [CI/CD](#cicd)
- [开发约定](#开发约定)
- [待做任务列表（TODO / Roadmap）](#待做任务列表todo--roadmap)
- [License](#license)

---

## 项目简介

**homeTier** 是一款跨平台（Windows / macOS / Linux 桌面端，以及 Android / iOS 移动端）的虚拟局域网应用。它基于 [EasyTier](https://github.com/EasyTier/EasyTier) 组网内核，为多个设备构建加密的 P2P 虚拟网络（空间），并在其上提供聊天、语音、屏幕共享、文件传输、局域网应用访问与分布式配置存储等能力。桌面端功能完整；移动端可构建安装包，VPN 隧道已接入平台原生实现，但媒体类能力仍在补齐（详见「平台支持与已知限制」）。

当前版本 **0.1.0**（`package.json:4`、`src-tauri/Cargo.toml:3`、`src-tauri/tauri.conf.json:33`），应用标识 `com.hometier.app`。

**三大运行模式：**

| 模式 | 入口 | 说明 |
|---|---|---|
| 桌面 GUI | `homeTier`（默认） | Tauri 原生窗口 + WebView 前端，daemon 以子进程运行（macOS 经 osascript 提权） |
| 守护进程 | `homeTier --daemon` | 无头后台进程，提供 TCP IPC 服务，管理 EasyTier 网络 |
| 服务器模式 | `homeTier --server` | 单进程 axum HTTP 服务器，提供 Web 管理界面 + REST/WS API，内嵌 daemon |

---

## 技术栈

### 后端

| 组件 | 选型 |
|---|---|
| 语言 | Rust 2021 edition（MSRV 1.75） |
| 桌面框架 | Tauri 2.x（插件：shell / process / clipboard / global-shortcut / os / notification / dialog / single-instance） |
| 异步运行时 | Tokio（full） |
| 组网内核 | 内置 EasyTier 2.6.4（`src-tauri/resources/easytier_lib/easytier`） |
| HTTP 服务器 | axum 0.8（服务器模式 REST API + WS） |
| HTTP 代理 | hyper 1.x + http-body-util（内嵌 iframe 代理） |
| 数据库 | SQLite（rusqlite bundled，自动迁移） |
| WebRTC | webrtc 0.11（语音 / 屏幕共享） |
| 安全 | aes-gcm（AES-256-GCM）、pbkdf2（210k 次迭代）、sha2、hmac、zstd（压缩） |
| 日志 | 自定义日志系统：内存环形缓冲 / 文件轮转 / JSON stdout / syslog / 转发 daemon 五类后端 |

> **依赖 feature 说明**：`src-tauri/Cargo.toml` 不存在顶层 feature 开关，`[features]` 仅有 `default = ["custom-protocol"]`；`wireguard` / `websocket` / `tun` / `socks5` / `kcp` / `quic` / `zstd` 是 vendored `easytier` 依赖的 feature，仅在 android / ios target 下启用（桌面端以空 features 引入，`src-tauri/Cargo.toml:102-107`）。

### 前端

| 组件 | 选型 |
|---|---|
| 框架 | React 18 + TypeScript 5.5（strict） |
| 构建 | Vite 5（dev 端口 1420，strictPort） |
| 状态管理 | Zustand 4（13 个 store） |
| 样式 | Tailwind CSS 3 + Radix Themes 3 |
| 路由 | React Router 6 |
| 国际化 | react-i18next（zh / zh-TW / en，默认 zh） |
| 包管理 | pnpm 9+（必须） |

---

## 架构总览

### 进程拓扑

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

- **双进程模型**：桌面 GUI 与 root 权限的 `daemon` 是两个独立进程（服务器模式则把 daemon 内嵌为同进程任务）。GUI 通过 `127.0.0.1:15889` 长度前缀 JSON IPC 与 daemon 通信；提权方式按平台区分：macOS 走 `osascript`、Windows 走 UAC、Linux 走 `pkexec`。`--daemon` / `--server` / `--elevated` 等参数解析见 `src-tauri/src/main.rs:64-139`。
- **空间互斥**：同一时间只运行一个 EasyTier 网络实例，`SpaceManager::connect` 会先断开当前连接。
- **日志来源切换**：daemon 以独立进程运行时，其日志面板通过 `source=daemon` 选择来源；daemon stdout 落在 `{app_data_dir}/daemon.log`，与 GUI 日志各自独立。
- **代理路由模型**：不再为每个对端下发应用层 `/32` 路由，而是以 `proxy_cidrs` 集中描述代理网段（`src-tauri/src/easytier/config.rs:257-303`）。原生 EasyTier(stock) 节点跨 `/24` 互联时需在 stock 侧手动配置 `proxy_cidr`。

### 核心模块

| 模块 | 职责 |
|---|---|
| `app/` | Tauri 生命周期粘合：`setup`（初始化 DB / daemon / 代理 / 托盘）、`exit` 清理、窗口显隐、提权标记 |
| `commands/` | 115 个已注册 `#[tauri::command]`，24 个模块（space / network / chat / file / voice / screen / proxy / config / easytier / daemon / log / tray / signal 等） |
| `daemon/` | 无头守护进程：TCP IPC 服务器、easytier-core 生命周期管理、S5 GUI 看门狗、优雅退出 |
| `space/` | 空间编排中枢：创建/加入/离开/连接/断开、peer 发现、聊天/语音/屏幕/文件服务器生命周期、加密分享链接 |
| `easytier/` | EasyTier 管理器：RPC 驱动 `easytier-core`（桌面）或进程内 launcher（移动端）、TOML 配置生成、二进制下载/升级 |
| `chat/` | P2P 聊天：每空间 HTTP 服务器 + 向 peer 广播，消息 HMAC 签名校验 |
| `voice/` `screen/` | WebRTC 语音/屏幕共享引擎 + 信令服务器（端口 18100+ / 18200），媒体面由前端实现 |
| `file/` | P2P 文件传输：zstd 压缩 + 可选 AES 加密、流式收发带进度、HTTP 文件服务器（19000 + space_id % 1000） |
| `proxy/` | 内嵌 HTTP 代理（127.0.0.1 随机端口）：CORS / iframe 绕过 / HTTPS 隧道 / URL 重写 / WebSocket 隧道 / `__proxy__` 本地 HTTP 代理 |
| `server/` | 服务器模式：axum 路由（`/api/cmd/*` 68 条 `.route(`，含 2 条 WebSocket）、静态资源（内嵌 dist）、Cookie 鉴权、TLS、事件总线 |
| `config_store/` | P2P 分布式配置存储（TCP 9877）：版本化文件 + 校验和 + 去重写队列 |
| `db/` | SQLite 持久化（10 张表），启动自动迁移 |
| `log/` | 统一日志系统与 `log_info!` / `log_warn!` / `log_error!` / `log_debug!` 宏 |
| `crypto/` | AES-256-GCM + PBKDF2-HMAC-SHA256（210k 迭代）、SHA-256、HMAC 签名 |
| `platform/` | `PlatformAdapter` 平台抽象（配置/日志目录）、机器标识读取 |

> 命令统计口径：`tauri::generate_handler!` 注册 **115** 个（`src-tauri/src/lib.rs:53-198`）；`#[tauri::command]` 属性共 **116** 个，多出的 `detect_lan_subnets`（`src-tauri/src/commands/space.rs:112`）仅通过 JNI 插件调用、未注册到 Tauri。

### 三种模式差异

| 维度 | 桌面 GUI | 守护进程 daemon | 服务器 Server |
|---|---|---|---|
| 前端 | Tauri WebView | 无 | 任意浏览器（axum 托管 `dist/`） |
| daemon 位置 | 独立子进程（macOS 提权） | 自身 | 同进程内嵌 |
| 命令接口 | `invoke()` 115 个 command | TCP IPC | REST `/api/cmd/*` + WS |
| 实时事件 | Tauri `listen("new_message")` | — | WS `/ws/events` + `/ws/signal/{spaceId}` |
| 日志 | 内存 + 转发 | IPC WriteLog | JSON stdout + 文件 + syslog |
| 配置 | `{app_data_dir}/homeTier.conf` | `{data_dir}/homeTier.conf` | `{server-dir}/homeTier.conf` + `server.conf` |

---

## 快速开始

### 环境要求

| 工具 | 版本 |
|---|---|
| Rust | 1.75+（vendored EasyTier 需 1.95 编译） |
| Node.js | 18+ |
| pnpm | 9+（**必须**：仓库使用 `pnpm-workspace.yaml` + `pnpm-lock.yaml`，根目录 `package-lock.json` 已过时，不要用 npm/yarn） |
| Tauri CLI | 2.x（`cargo install tauri-cli --version "^2"`） |

平台编译工具链（Tauri 2 官方要求）：Windows 需 VS Build Tools（含 C++ 桌面工作负载）；macOS 需 Xcode；Linux 需 `libwebkit2gtk-4.1-dev`、`libappindicator3-dev` 等。

### 开发

```bash
pnpm install                     # 安装前端依赖
pnpm tauri dev                   # 启动 Tauri 开发（Vite:1420 + Rust）
```

仅调试前端（无原生窗口）：

```bash
pnpm dev                         # Vite dev server on http://localhost:1420
```

### 代码检查

```bash
pnpm lint                        # eslint src/
pnpm lint:fix                    # eslint src/ --fix
```

### 构建

```bash
pnpm build                       # tsc --noEmit && vite build（前端静态资源）
pnpm build:server                # 服务器模式静态资源（与 pnpm build 等价）
pnpm tauri build                 # 平台安装包（tsc + vite build + Rust release）
```

各平台打包脚本：

| 脚本 | 目标 |
|---|---|
| `pnpm tauri:build:macos` | macOS arm64 dmg |
| `pnpm tauri:build:macos:x86` | macOS x86_64 dmg |
| `pnpm tauri:build:windows` | Windows x86_64 msi |
| `pnpm tauri:build:linux` | Linux x86_64 deb |
| `pnpm tauri:build:linux:arm` | Linux aarch64 deb |
| `pnpm tauri:build:appimage` | AppImage |
| `pnpm tauri:build:android` | Android APK（arm64 / armv7 / x86_64） |
| `pnpm tauri:build:docker` | Docker 镜像资源（`tauri build --no-bundle`） |

### 后端类型检查

```bash
cd src-tauri && cargo check --all-targets    # 需要本机 cc linker（Windows/macOS 自带）
```

> **Linux 宿主无 cc linker 时**：使用项目 Docker 开发容器验证编译：
> ```bash
> docker exec -w /workspace/homeTier/src-tauri rust-dev cargo check --bin homeTier
> ```

### 子路径部署

前端静态资源公共前缀由 `VITE_PUBLIC_BASE` 控制（`.env.example:7`、`vite.config.ts:5-13`），默认 `/`。部署到 nginx 子路径/反向代理子目录时改为如 `/hometier/` 的前缀；**桌面/移动端构建必须保持 `/`**（Tauri 以根路径加载 dist）。

### 移动端辅助脚本

`scripts/` 下提供移动端构建辅助脚本：`download-npcap-dlls.sh`、`fix-android-build-gradle.sh`、`fix-android-mainactivity.sh`、`mobile-permissions.sh`、`verify-android-injection.sh`。

---

## 服务器模式

服务器模式将 homeTier 变成单进程 Web 服务：一个进程同时提供 Web 管理界面、REST/WS API 与内嵌 daemon（默认监听 `0.0.0.0:9339`）。

### CLI 参数

| 参数 | 默认值 | 说明 |
|---|---|---|
| `--server` | — | 启用服务器模式 |
| `--server-bind` | `0.0.0.0` | 监听地址 |
| `--server-port` | `9339` | 监听端口 |
| `--server-dir` | `./homeTier-data` | 数据目录（DB、server.conf、easytier 配置） |
| `--server-resource-dir` | 内置 | 资源目录（easytier-core 兜底二进制） |
| `--server-static-dir` | 内嵌 dist | 前端静态资源目录（未提供时用编译期内嵌资源） |

> `SERVER_BIND` / `SERVER_PORT` / `SERVER_STATIC_DIR` 是服务器模式的**运行时默认值**（`0.0.0.0` / `9339` / `./dist`，`src-tauri/src/server/mod.rs:35-39`），它们**不在** `homeTier.conf.example` 中；后者提供的是 `DAEMON_IPC_PORT` / `EASYTIER_RPC_PORT` / `FILE_SERVER_PORT_BASE` / `DEFAULT_SPACE_IP` / `GITHUB_API` / `GITHUB_MIRROR` / `RELAY_NETWORK_PREFIX` / `LOG_ENABLED`。

### server.conf（数据目录内生成）

| 键 | 默认值 | 说明 |
|---|---|---|
| `SERVER_BIND` | `0.0.0.0` | 监听地址 |
| `SERVER_PORT` | `9339` | HTTP 端口 |
| `SERVER_STATIC_DIR` | `./dist` | 静态资源目录 |
| `SERVER_TLS` | `false` | 启用 TLS（需同时配置证书） |
| `SERVER_TLS_CERT` / `SERVER_TLS_KEY` | 空 | PEM 证书 / 私钥路径 |
| `SERVER_AUTH_SECRET` | 自动生成 | Cookie 鉴权 HMAC 密钥（32 字节 hex） |
| `SERVER_CORS_ORIGIN` | `*` | 允许的 CORS 来源（`*` 时禁用凭据） |
| `SERVER_PROXY_PREFIX` | `/proxy` | 内嵌代理前缀 |

### Docker 部署

项目根目录提供 `Dockerfile`（构建阶段 Node 22 + Rust，运行阶段 `debian:bookworm-slim`）：

```bash
docker build -t hometier .
docker run -d --name hometier --restart unless-stopped \
  -p 9339:9339 \
  -v hometier-data:/home/hometier/.local/share/homeTier \
  hometier
```

> 镜像默认 `EXPOSE 15888 15889 9339`，非 root 用户 `hometier` 运行，数据目录 `$HOME/.local/share/homeTier`。

### systemd 部署

参见 `deploy/hometier-server.service`：

```bash
install -d /opt/homeTier
pnpm build:server                 # 必须先构建前端静态资源
cp -r dist /opt/homeTier/dist
cp homeTier.conf.example /opt/homeTier/homeTier.conf
install -m 755 src-tauri/target/release/homeTier /opt/homeTier/homeTier
cp deploy/hometier-server.service /etc/systemd/system/
systemctl daemon-reload && systemctl enable --now hometier-server
```

---

## 配置与数据

### homeTier.conf

应用配置为 `.env` 风格 `KEY=VALUE` 文件，支持热加载（2s mtime 轮询，修改即生效）。优先级：**运行时配置 > 模板默认值（`homeTier.conf.example`）> 内置默认值**。模板位于项目根 `homeTier.conf.example`，首次启动复制到数据目录。

| 键 | 默认值 | 说明 |
|---|---|---|
| `DAEMON_IPC_PORT` | `15889` | daemon IPC 端口 |
| `EASYTIER_RPC_PORT` | `15888` | easytier-core RPC 端口 |
| `FILE_SERVER_PORT_BASE` | `19000` | 文件服务器端口基数（实际 = 基数 + space_id % 1000） |
| `DEFAULT_SPACE_IP` | `10.144.144.10` | 新建空间默认虚拟 IPv4 |
| `GITHUB_API` | GitHub Releases API | EasyTier 版本检查 |
| `GITHUB_MIRROR` | `https://ghproxy.top` | 下载镜像前缀（留空直连 GitHub） |
| `RELAY_NETWORK_PREFIX` | `homeTier_` | 中继网络前缀 |
| `LOG_ENABLED` | `1` | 日志开关 |

> 服务器模式的 `SERVER_*` 键写入数据目录内的 `server.conf`，而非 `homeTier.conf`（见「服务器模式」）。

### 数据库

SQLite 文件位于 `{app_data_dir}/homeTier.db`（服务器模式为 `{server-dir}/homeTier.db`），启动自动迁移（`src-tauri/src/db/migrations.rs`），共 10 张表：

| 表 | 说明 |
|---|---|
| `users` | 本机用户（机器标识） |
| `spaces` | 空间（含 `network_secret`、`config_json`） |
| `members` | 成员（虚拟 IP、在线状态、是否 owner） |
| `messages` | 聊天消息（含发送状态） |
| `files` | 文件记录 |
| `settings` | 键值设置 |
| `proxy_cookies` | 内嵌代理 Cookie 落库 |
| `space_apps` | 空间内应用（iframe 浏览器用） |
| `acl_rules` | ACL 规则 |
| `port_forward_rules` | 端口转发规则 |

### 端口约定

| 端口 | 用途 |
|---|---|
| `15889` | daemon TCP IPC（可配置） |
| `15888` | easytier-core RPC（可配置） |
| `9877` | 分布式配置存储 TCP（P2P） |
| `9339` | 服务器模式 HTTP（可配置） |
| `19000 + space_id % 1000` | 文件传输 HTTP 服务器 |
| `18100 + space_id % 1000` | 语音信令服务器 |
| `18200 + space_id % 1000` | 屏幕共享信令服务器 |
| `127.0.0.1` 随机端口 | 内置 HTTP 代理 |

### 分享链接

格式：`homeTier://join?v=1&d={base64url}`。载荷流程：ShareInfo **二进制编码（小端序、单字节长度前缀、可选字段 bitmask）** → **自适应压缩（zstd level 3；压缩后不小于原文则走 raw）** → **AES-256-GCM 加密**（密钥 = SHA-256(配置 `SHARE_LINK_SECRET`，默认 `homeTier-qr-v1`)）→ base64url（无 padding）。聊天消息使用空间 `network_secret` 做 HMAC-SHA256 签名校验；密码保护文件使用 PBKDF2 派生密钥加密。

---

## 前后端通信契约

### Tauri 命令（桌面）

- **115 个**已注册 `#[tauri::command]`，分布于 `src-tauri/src/commands/` 24 个模块，在 `src-tauri/src/lib.rs:53-198` 注册；另有 116 个 `#[tauri::command]` 属性，其中 `detect_lan_subnets` 仅 JNI 调用、未注册。
- 前端通过 `src/utils/api.ts:14` 统一封装：运行时检测 `__TAURI_INTERNALS__` 自动选择 **Tauri `invoke()`**（`src/utils/api/tauri.ts`，约 70 个 `invoke`）或 **REST/WS**（`src/utils/api/web.ts`）实现，业务代码无感。
- 主要命令域：space（14）、config_store（8）、file（6）、util/app（5+5）、voice/screen（4+4）、proxy（4）、ACL/端口转发（4+4）、easytier（4）、daemon/config（4+4）、network/log（3+3）、chat（2）、tray/signal（1+1）。

### 服务器模式 REST + WS

- REST：`/api/cmd/*`（ping / space / chat / network / log / config / file / proxy / easytier / config-store 等），`src-tauri/src/server/routes.rs` 共 **68 条 `.route(`**，JSON + cookie 鉴权。
- WebSocket：`/api/cmd/ws/events`（全局事件流）、`/api/cmd/ws/signal/{spaceId}`（WebRTC 信令转发）；两条均为 `routes.rs` 中的 WebSocket 路由。
- 事件类型（`server/event.rs`）：SpaceCreated/Deleted/Updated、MemberJoined/Left、MessageSent、FileShared、ScreenShareStarted/Stopped、VoiceCallStarted/Stopped、PeerConnected/Disconnected、ConfigChanged、SystemLog。

### 前端事件（桌面）

`new_message`（聊天/信令）、`tray-navigate`（托盘导航）、`daemon-ready`（daemon 就绪）、`easytier-download-progress`（升级进度）、`config:changed`（配置热更新）。

### WebRTC 信令（关键设计）

语音/屏幕共享的**信令控制面复用聊天消息通道**：`msg_type="signal"` 携带 `SignalEnvelope {kind, type, from, to, data}`，经 `realtime.ts` 分发到 `signal.ts`，再路由至 `voice.ts` / `screen.ts`（浏览器端全网格 WebRTC，无后端媒体面）。确定性 offerer：虚拟 IP 字典序较小者为 offerer。

### 类型同步

| 后端 | 前端 | 说明 |
|---|---|---|
| `src-tauri/src/types.rs` | `src/types/index.ts` | Space / Member / Message / FileInfo 等并行类型，手动同步 |
| `easytier/config.rs` 的 `NetworkConfig` | `src/types/network.ts` | 网络配置镜像（含 `DEFAULT_NETWORK_CONFIG()`） |

---

## 目录结构

```
homeTier/
├── src/                        # 前端 React/TS
│   ├── components/             # 按功能域组织的 UI（Layout/Space/Chat/Voice/...）
│   ├── stores/                 # Zustand（13 个 store：space/settings/file/chat/voice/screen/appTabs/...）
│   ├── services/               # realtime / signal / voice / screen / shortcuts
│   ├── utils/                  # api.ts（双模式入口）+ api/{tauri,web,core}.ts + 工具
│   ├── i18n/                   # locales（zh / zh-TW / en）
│   ├── types/                  # index.ts（领域模型）+ network.ts（NetworkConfig）
│   └── hooks/  enum/  styles/
├── src-tauri/                  # 后端 Rust
│   ├── src/                    # 见"核心模块"表
│   ├── src/commands/           # 24 个命令模块（见"前后端通信契约"）
│   ├── resources/easytier_lib/easytier   # 内置 EasyTier 2.6.4（vendored，只读）
│   ├── resources/bin/          # easytier-core 兜底二进制
│   ├── tauri.conf.json         # Tauri 配置（identifier: com.hometier.app, v0.1.0）
│   └── Cargo.toml
├── docs/                       # 本地设计文档（不入远程仓库，GitHub 不显示）
├── deploy/hometier-server.service  # systemd 部署单元
├── Dockerfile                  # 服务器模式容器镜像
├── homeTier.conf.example       # 配置模板
└── package.json / pnpm-lock.yaml
```

> `src-tauri/resources/easytier_lib/` 为 vendored EasyTier 源码，**只读**；仅允许修改 `edition` / `rust-version`，其余不得改动。

### 前端路由

| 路由 | 页面 |
|---|---|
| `/` | 空间列表（连接/分享/配置/删除） |
| `/space/:id` | 空间主页（网络统计 + 应用启动器） |
| `/space/:id/chat` `/voice` `/screen` `/files` `/logs` | 聊天 / 语音 / 屏幕共享 / 文件 / 日志 |
| `/space/:id/app/:appId` | 应用 iframe 标签深链 |
| `/settings` | 设置（基础 / EasyTier / 配置 / 日志 四个页签） |
| `*` | 404 |

共 9 条具名路由 + `*`（`src/App.tsx:252-263`）。

---

## 功能特性

- **空间组网**：创建 / 加入 / 连接 / 断开空间，生成加密分享链接与二维码，空间内自动发现 peer 并建立 Mesh 路由。
- **网络概览**：成员列表、Mesh 路由、流量统计、ACL 规则、端口转发规则。
- **带签名的聊天**：消息使用空间 `network_secret` 做 HMAC-SHA256 签名校验，经 `new_message` 事件推送，乐观更新 + 去重 + 虚拟化列表。
- **P2P 文件传输**：zstd 压缩 + 可选 AES 加密，SHA-256 校验 + 流式收发带进度、断点续传进度恢复。
- **桌面 WebRTC 语音**：由前端实现（`src/services/voice.ts`），全网格直连、RMS 语音活动检测（VAD）、麦克风/扬声器控制。
- **桌面屏幕共享**：由前端实现（`src/services/screen.ts`），邀请制 ACL、三档画质（smooth/standard/hd，maxBitrate 控制）。
- **信令复用聊天通道**：语音/屏幕的 `signal` 消息经 `new_message` 走聊天通道转发，无独立媒体服务器。
- **应用导航页 + 内置反向代理**：iframe 访问空间内任意 HTTP/HTTPS/WSS 应用，自签 CA、绕过 CSP/X-Frame-Options、Cookie 落库，最多 10 个 LRU 标签页。
- **双来源日志**：GUI 与 daemon 日志各自独立，日志面板可切换来源。
- **配置中心 + P2P 配置同步**：`config_store` 版本化配置（TCP 9877）与远端配置同步，防版本回滚。
- **EasyTier 版本管理**：检查 / 下载 / 升级 easytier-core，带进度事件。
- **应用自更新**：走 GitHub Release（未接入 Tauri 官方 updater，见 `src-tauri/src/commands/update_app.rs`）。
- **托盘 / 后台运行**：关闭窗口隐藏到托盘，托盘菜单随语言/状态热同步。
- **三种运行形态**：桌面 GUI / `--server` / 移动端。
- **多语言**：中文（默认）/ 繁体中文 / English。

---

## 平台支持与已知限制

| 平台 | 状态 |
|---|---|
| Windows | 完整桌面 + UAC 提权，内置 Npcap / WinDivert / Wintun DLL |
| macOS | 完整桌面；GUI 不提权，daemon 经 `osascript` 提权 |
| Linux | 完整桌面 + deb / AppImage；提权走 `pkexec` |
| Android | 可构建 APK；VPN 走 Kotlin `HomeTierVpnService` 插件；语音/屏幕原生桥存在但前端未接线；`get_vpn_status` 为占位 |
| iOS | 已有 NetworkExtension 静态库与 Xcode 注入脚本，但缺 host app 桥接，媒体链路为 TODO |

补充说明：

- 平台适配器只做配置/日志目录解析（`src-tauri/src/platform/mod.rs:19`）。
- 未接入 Tauri 官方 updater，应用自更新走 GitHub Release（`src-tauri/src/commands/update_app.rs`）。
- Android/iOS 的 VPN 接口 IP 必须等于 EasyTier 节点身份 IP，且不得使用 `10.144.144.1`（移动端兜底 `.10`）。
- 移动端媒体与真机验证缺口详见「待做任务列表」。

---

## CI/CD

- **`.github/workflows/ci.yml`**：`frontend` 任务执行 `pnpm lint` + `pnpm build`；`backend` 任务安装 webkit2gtk 等系统依赖、`mkdir -p dist` 后执行 `cargo check --all-targets`。**目前没有任何测试任务**（仓库已有约 35 个 Rust `#[test]` / `#[tokio::test]`，见 P3 条目）。
- **`.github/workflows/release.yml`**：`fetch-easytier` 下载并校验各平台 easytier-core，随后构建 macOS / Windows / Linux / AppImage / Android / Docker 产物。iOS 默认跳过（见 `未完成平台隔离，默认跳过` 开关 `build_ios`）。
- **`.github/actions/setup-build/action.yml`**：公共构建环境（pnpm / Node 22 / Rust 工具链 / 缓存 / easytier 下载 / Linux 依赖安装），注意其**不含 checkout**，调用方需先 `actions/checkout@v4`。

---

## 开发约定

- **必须使用 pnpm**：仓库有 `pnpm-workspace.yaml` 与 `pnpm-lock.yaml`；根目录 `package-lock.json` 已过时，不要使用 npm/yarn。
- 提交前运行 `pnpm lint`。
- **禁止修改 `src-tauri/resources/easytier_lib/`**：这是 vendored EasyTier，只读；仅允许修改其 `edition` / `rust-version`。
- 后端日志使用自定义 `log_*!` 宏写入内存日志，而非直接 `println!` / stdout。
- GUI 与 daemon 是两个独立进程，日志各自独立。
- UI 统一用项目的 `Tip` 组件封装，而非直接引入 Radix `Tooltip`。
- `patch_config` 已修复（`src-tauri/src/easytier/mod.rs` 引入 JSON 伴随文件全量序列化），现可安全用于运行时改配置。

---

## 待做任务列表（TODO / Roadmap）

### P0（阻断/线上隐患）

| 任务 | 证据 | 影响 | 工作量 |
|---|---|---|---|
| iOS 系统级 VPN 启动桥接（host app → NetworkExtension） | `src-tauri/src/commands/ios_vpn.rs:69-72` 只 emit `ios:start-vpn` 无接收方；`src/services/mobileVpn.ts:143` 无条件调用仅 Android 存在的 `plugin:hometiervpnservice\|start_vpn`；`src-tauri/gen-scripts/ios/` 只有 NE extension 文件，无 host `@main` / AppDelegate / `NETunnelProviderManager.startVPNTunnel` 调用方 | iOS 上 VPN 无法建立 | L |

> 依赖 Xcode 工程 + Apple Developer 账号 + 真机（iOS 桥接）。

### P1（核心功能缺失）

| 任务 | 证据 | 影响 | 工作量 |
|---|---|---|---|
| 移动端语音通话实现 | `src-tauri/src/voice/mobile/android.rs:75-160` 的 JNI 指向仓库中不存在的 Kotlin `VoiceManager`；`src-tauri/src/voice/mobile/ios.rs` 全为 TODO；`src/stores/mobileVoiceStore.ts:46-49` 的 invoke 被注释 | 移动端无语音 | L |
| 移动端屏幕共享实现 | `src-tauri/scripts/android/screen/ScreenShareManager.kt:90-104` 创建 VirtualDisplay 时 `Surface = null`（无采集）；`src-tauri/src/screen/mobile/android.rs:262-275` 仅打日志；`src-tauri/src/screen/mobile.rs:192` TODO ReplayKit；`src/stores/mobileScreenStore.ts:31-46` invoke 被注释 | 移动端无屏幕共享 | L |

### P2（平台补齐）

| 任务 | 证据 | 影响 | 工作量 |
|---|---|---|---|
| 移动端 easytier-core 更新/守护为 stub | `src-tauri/src/commands/mobile_vpn.rs:16` 占位 `get_vpn_status`；移动端隐藏更新入口（`src/components/Settings/EasyTierVersionManager.tsx:114`） | 移动端无法管理内核版本 | M |
| 移动端全局快捷键为空壳（入口已隐藏） | `src/components/Settings/SettingsPage.tsx:314` | 移动端无快捷键 | S |
| 移动端聊天/文件传输需真机验证 | `src-tauri/src/chat/`、`src-tauri/src/file/` 无移动端分支 | 移动端聊天/传输未验证 | S |
| Windows ARM64 桌面包缺失 | `docs/workflow.md:298` | 无 Windows ARM64 安装包 | M |
| macOS 公证 + Windows 代码签名 | `docs/workflow.md:136`、`:254-271`（暂缓） | 首次安装有安全提示 | L |
| iOS NE 签名与上架合规（TUN fd 走 KVC 私有 API 的风险） | `docs/mobile_vpn.md:1152-1159` | 可能被 App Store 拒审 | L |
| server 模式 P2P 传输进度查询未实现 | `src-tauri/src/server/routes.rs:1566` 返回 NOT_IMPLEMENTED | Web 模式无法查询传输进度 | S |
| 原生 EasyTier(stock) → homeTier 跨 /24 需 stock 侧手动配 `proxy_cidr` | `src-tauri/src/easytier/config.rs:257-303` | 需补用户 FAQ | S |

### P3（工程化/文档/发布）

| 任务 | 证据 | 影响 | 工作量 |
|---|---|---|---|
| CI 未运行任何测试 | `.github/workflows/ci.yml:12-60` 只有 `pnpm lint` / `pnpm build` + `cargo check --all-targets`，而仓库已有约 35 个 Rust `#[test]` / `#[tokio::test]` | 回归风险 | M |
| 移动端真机测试 checklist 未执行 | `docs/MOBILE_VPN_TEST_CHECKLIST.md` 结果栏为空 | 移动端质量未知 | M |
| 死代码 | `src-tauri/src/voice/interop.rs`、`voice/opus.rs` 未参与编译（`src-tauri/src/voice/mod.rs:1-6`），`rusty-opus`（`src-tauri/Cargo.toml:38`）未被使用 | 维护噪音 | S |
| 文档漂移 | 配置中心端口（本次 README 已修）、`AGENTS.md` 命令数 151→115、`AGENTS.md` 引用不存在的 `src/types/config.ts`（实际 `src/types/` 仅有 `index.ts`、`network.ts`） | 误导开发者 | S |
| 移动端 AppBrowser 文档过时 | 本地代理在移动端无条件启动（`src-tauri/src/app/setup.rs:466`） | 文档与实现不一致 | S |

> 已完成项不再列入；`easytier_lib/` 内的上游 TODO 已排除。

---

## License

本项目基于 **GPL-3.0-or-later** 许可证（`package.json:6`），仓库根目录同时提供 `LICENSE` 与 `GPL-3.0 license` 两个文件。相关依赖遵循其各自的开源许可（EasyTier: Apache-2.0）。
