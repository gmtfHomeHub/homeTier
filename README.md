# homeTier

> A cross-platform virtual LAN application powered by EasyTier

[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-24C8D8)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18-blue)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.5%20strict-3178C6)](https://www.typescriptlang.org)
[![EasyTier](https://img.shields.io/badge/EasyTier-2.6.4-green)](https://github.com/EasyTier/EasyTier)

**[中文](README_CN.md)** | **English**

---

## Table of Contents

- [Introduction](#introduction)
- [Tech Stack](#tech-stack)
- [Architecture Overview](#architecture-overview)
- [Quick Start](#quick-start)
- [Server Mode](#server-mode)
- [Configuration & Data](#configuration--data)
- [Frontend–Backend Contract](#frontendbackend-contract)
- [Directory Structure](#directory-structure)
- [Feature Highlights](#feature-highlights)
- [Documentation](#documentation)

---

## Introduction

**homeTier** is a cross-platform (Windows / macOS / Linux, with Android/iOS stubs) virtual LAN application built on the [EasyTier](https://github.com/EasyTier/EasyTier) networking kernel. It establishes encrypted P2P virtual networks ("spaces") among devices, on top of which it provides chat, voice, screen sharing, file transfer, LAN app browsing and distributed config storage.

**Three runtime modes:**

| Mode | Entry | Description |
|---|---|---|
| Desktop GUI | `homeTier` (default) | Tauri native window + WebView frontend, daemon as subprocess (elevated via osascript on macOS) |
| Daemon | `homeTier --daemon` | Headless background process providing TCP IPC service, manages EasyTier networks |
| Server | `homeTier --server` | Single-process axum HTTP server providing Web UI + REST/WS API with embedded daemon |

---

## Tech Stack

### Backend

| Component | Choice |
|---|---|
| Language | Rust 2021 edition (MSRV 1.75) |
| Desktop framework | Tauri 2.x (plugins: shell / process / clipboard / global-shortcut / os / notification / dialog / single-instance) |
| Async runtime | Tokio (full) |
| Networking kernel | Built-in EasyTier 2.6.4 (`src-tauri/resources/easytier_lib/easytier`), features: `wireguard` `websocket` `tun` `socks5` `kcp` `quic` `zstd` |
| HTTP server | axum 0.8 (server mode REST API + WS) |
| HTTP proxy | hyper 1.x + http-body-util (embedded iframe proxy) |
| Database | SQLite (rusqlite bundled, auto-migration) |
| WebRTC | webrtc 0.11 (voice / screen sharing) |
| Security | aes-gcm (AES-256-GCM), pbkdf2 (210k iterations), sha2, hmac, zstd (compression) |
| Logging | Custom log system: in-memory ring buffer / file rotation / JSON stdout / syslog / daemon forwarding |

### Frontend

| Component | Choice |
|---|---|
| Framework | React 18 + TypeScript 5.5 (strict) |
| Build | Vite 5 (dev port 1420, strictPort) |
| State management | Zustand 4 (9 stores) |
| Styling | Tailwind CSS 3 + Radix Themes 3 |
| Routing | React Router 6 |
| i18n | react-i18next (zh / zh-TW / en, default zh) |
| Package manager | pnpm 9+ |

---

## Architecture Overview

### Process Topology

```
┌──────────────────────────┐      ┌──────────────────────────┐
│  GUI / Server process    │      │  Server mode (single)    │
│  (Tauri / axum)          │      │  run_server()            │
│        │                 │      │  embedded daemon (task)  │
│        │ TCP IPC :15889  │      │        │                 │
│        ▼                 │      │        │                 │
│  daemon subprocess       │      │  daemon (in-process)     │
│  (elevated root/admin)   │      │        │ gRPC/TCP :15888 │
│        │ gRPC/TCP :15888 │      │        ▼                 │
│        ▼                 │      │  easytier-core           │
│  easytier-core           │      │  (TUN virtual NIC)       │
│  (TUN virtual NIC)      │      │                          │
└──────────────────────────┘      └──────────────────────────┘
```

- **Space mutual exclusion**: Only one EasyTier network instance runs at a time; `SpaceManager::connect` disconnects the current connection first.
- **Frontend–backend decoupling**: GUI and daemon communicate via `127.0.0.1:15889` length-prefixed JSON IPC; server mode embeds the daemon in-process, reusing the same IPC protocol.

### Core Modules

| Module | Responsibility |
|---|---|
| `app/` | Tauri lifecycle glue: `setup` (init DB/daemon/proxy/tray), `exit` cleanup, window visibility, elevation flags |
| `commands/` | 80 `#[tauri::command]`s across 18 modules (space / network / chat / file / voice / screen / proxy / config / easytier / daemon / log / tray / signal etc.) |
| `daemon/` | Headless daemon: TCP IPC server, easytier-core lifecycle management, GUI watchdog, graceful shutdown |
| `space/` | Space orchestration: create/join/leave/connect/disconnect, peer discovery, chat/voice/screen/file server lifecycle, encrypted share links |
| `easytier/` | EasyTier manager: RPC-driven `easytier-core` (desktop) or in-process launcher (mobile), TOML config generation, binary download/upgrade |
| `chat/` | P2P chat: per-space HTTP server + broadcast to peers, HMAC-signed messages |
| `voice/` `screen/` | WebRTC voice/screen sharing engine + signaling server (ports 18100+ / 18200) |
| `file/` | P2P file transfer: zstd compression + optional AES encryption, streaming with progress, HTTP file server (19000 + space_id % 1000) |
| `proxy/` | Embedded HTTP proxy (127.0.0.1 random port): CORS/iframe bypass/HTTPS tunnel/URL rewrite/WebSocket tunnel/`__proxy__` local HTTP proxy |
| `server/` | Server mode: axum routes (`/api/cmd/*` ~60 routes), WebSocket, static assets (embedded dist), Cookie auth, TLS, event bus |
| `config_store/` | P2P distributed config store (TCP 9877): versioned files + checksums + dedup write queue |
| `db/` | SQLite persistence (8 tables), auto-migration on startup |
| `log/` | Unified logging system with `log_info!` / `log_warn!` / `log_error!` / `log_debug!` macros |
| `crypto/` | AES-256-GCM + PBKDF2-HMAC-SHA256 (210k iterations), SHA-256, HMAC signing |
| `platform/` | `PlatformAdapter` platform abstraction (config/log directories), machine ID |

### Mode Comparison

| Dimension | Desktop GUI | Daemon | Server |
|---|---|---|---|
| Frontend | Tauri WebView | None | Any browser (axum serves `dist/`) |
| Daemon location | Separate subprocess (elevated on macOS) | Itself | In-process embedded |
| Command interface | `invoke()` 80 commands | TCP IPC | REST `/api/cmd/*` + WS |
| Realtime events | Tauri `listen("new_message")` | — | WS `/ws/events` + `/ws/signal/{spaceId}` |
| Logging | In-memory + forwarding | IPC WriteLog | JSON stdout + file + syslog |
| Config | `{app_data_dir}/homeTier.conf` | `{data_dir}/homeTier.conf` | `{server-dir}/homeTier.conf` + `server.conf` |

---

## Quick Start

### Prerequisites

| Tool | Version |
|---|---|
| Rust | 1.75+ (vendored EasyTier requires 1.95 to build) |
| Node.js | 18+ |
| pnpm | 9+ |
| Tauri CLI | 2.x (`cargo install tauri-cli --version "^2"`) |

Platform build toolchains (Tauri 2 official requirements): Windows needs VS Build Tools (with C++ desktop workload); macOS needs Xcode; Linux needs `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, etc.

### Development

```bash
pnpm install                     # Install frontend dependencies
pnpm tauri dev                   # Launch Tauri dev (Vite:1420 + Rust)
```

Frontend-only debugging (no native window):

```bash
pnpm dev                         # Vite dev server on http://localhost:1420
```

### Build

```bash
pnpm tauri build                 # Production build (tsc --noEmit && vite build + Rust release + platform installer)
```

### Backend Type Check

```bash
cd src-tauri && cargo check      # Requires local cc linker (Windows/macOS included)
```

> **Linux host without cc linker**: Use the project Docker dev container:
> ```bash
> docker exec -w /workspace/homeTier/src-tauri rust-dev cargo check --bin homeTier
> ```

---

## Server Mode

Server mode turns homeTier into a single-process web service: one process provides the Web UI, REST/WS API, and an embedded daemon (default `0.0.0.0:9339`).

### CLI Flags

| Flag | Default | Description |
|---|---|---|
| `--server` | — | Enable server mode |
| `--server-bind` | `0.0.0.0` | Listen address |
| `--server-port` | `9339` | Listen port |
| `--server-dir` | `./homeTier-data` | Data directory (DB, server.conf, easytier configs) |
| `--server-resource-dir` | built-in | Resource directory (easytier-core fallback binary) |
| `--server-static-dir` | embedded dist | Frontend static assets directory (falls back to compile-time embedded resources) |

### server.conf (generated in data dir)

| Key | Default | Description |
|---|---|---|
| `SERVER_BIND` | `0.0.0.0` | Listen address |
| `SERVER_PORT` | `9339` | HTTP port |
| `SERVER_STATIC_DIR` | `./dist` | Static assets directory |
| `SERVER_TLS` | `false` | Enable TLS (requires certificate configuration) |
| `SERVER_TLS_CERT` / `SERVER_TLS_KEY` | empty | PEM certificate / private key path |
| `SERVER_AUTH_SECRET` | auto-generated | Cookie auth HMAC key (32-byte hex) |
| `SERVER_CORS_ORIGIN` | `*` | Allowed CORS origins (credentials disabled when `*`) |
| `SERVER_PROXY_PREFIX` | `/proxy` | Embedded proxy prefix |

### Docker Deployment

The project root provides a `Dockerfile` (build stage: Node 22 + Rust, runtime: `debian:bookworm-slim`):

```bash
docker build -t hometier .
docker run -d --name hometier --restart unless-stopped \
  -p 9339:9339 \
  -v hometier-data:/home/hometier/.local/share/homeTier \
  hometier
```

> Image exposes `15888 15889 9339`, runs as non-root user `hometier`, data directory `$HOME/.local/share/homeTier`.

### systemd Deployment

See `deploy/hometier-server.service`:

```bash
install -d /opt/homeTier
cp -r dist /opt/homeTier/dist
cp homeTier.conf.example /opt/homeTier/homeTier.conf
install -m 755 src-tauri/target/release/homeTier /opt/homeTier/homeTier
cp deploy/hometier-server.service /etc/systemd/system/
systemctl daemon-reload && systemctl enable --now hometier-server
```

---

## Configuration & Data

### homeTier.conf

Application config is a `.env`-style `KEY=VALUE` file with hot-reload (2s mtime polling, changes apply immediately). Priority: **runtime config > template defaults (`homeTier.conf.example`) > built-in defaults**. Template is at project root `homeTier.conf.example`, copied to data directory on first launch.

| Key | Default | Description |
|---|---|---|
| `DAEMON_IPC_PORT` | `15889` | daemon IPC port |
| `EASYTIER_RPC_PORT` | `15888` | easytier-core RPC port |
| `FILE_SERVER_PORT_BASE` | `19000` | File server port base (actual = base + space_id % 1000) |
| `DEFAULT_SPACE_IP` | `10.144.144.10` | Default virtual IPv4 for new spaces |
| `GITHUB_API` | GitHub Releases API | EasyTier version check |
| `GITHUB_MIRROR` | `https://ghproxy.top` | Download mirror prefix (empty = direct GitHub) |
| `RELAY_NETWORK_PREFIX` | `homeTier_` | Relay network prefix |
| `LOG_ENABLED` | `1` | Logging toggle |

### Database

SQLite file at `{app_data_dir}/homeTier.db` (server mode: `{server-dir}/homeTier.db`), auto-migrated on startup (`src-tauri/src/db/migrations.rs`), 8 tables:

| Table | Description |
|---|---|
| `users` | Local users (machine identity) |
| `spaces` | Spaces (includes `network_secret`, `config_json`) |
| `members` | Members (virtual IP, online status, owner flag) |
| `messages` | Chat messages (includes send status) |
| `files` | File records |
| `settings` | Key-value settings |
| `space_apps` | In-space apps (for iframe browser) |
| `acl_rules` / `port_forward_rules` | ACL rules / port forwarding rules |

### Port Conventions

| Port | Usage |
|---|---|
| `15889` | daemon TCP IPC (configurable) |
| `15888` | easytier-core RPC (configurable) |
| `19000 + space_id % 1000` | File transfer HTTP server |
| `18100 + space_id % 100` | Voice signaling server |
| `18200` | Screen sharing signaling server |
| `9877` | Distributed config store TCP (P2P) |
| `9339` | Server mode HTTP (configurable) |

### Share Links

Format: `homeTier://join?v=3&d={base64url}`. Payload flow: ShareInfo serialized → **zstd compress (level 3)** → **AES-256-GCM encrypt** (key = SHA-256("homeTier-share-link-v2"), versioned for rotation) → base64url. Chat messages use the space `network_secret` for HMAC-SHA256 signature verification; password-protected files use PBKDF2-derived key encryption.

---

## Frontend–Backend Contract

### Tauri Commands (Desktop)

- **80** `#[tauri::command]`s across 18 modules in `src-tauri/src/commands/`, registered in `src-tauri/src/lib.rs`.
- Frontend unified wrapper at `src/utils/api.ts`: runtime detection of `__TAURI_INTERNALS__` auto-selects **Tauri `invoke()`** (`utils/api/tauri.ts`) or **REST/WS** (`utils/api/web.ts`) implementation — business code is agnostic.
- Main command domains: space (14), config_store (8), file (6), util/app (5+5), voice/screen (4+4), proxy (4), ACL/port-forward (4+4), easytier (4), daemon/config (4+4), network/log (3+3), chat (2), tray/signal (1+1).

### Server Mode REST + WS

- REST: `/api/cmd/*` (~60 routes: ping / space / chat / network / log / config / file / proxy / easytier / config-store etc.), JSON + cookie auth.
- WebSocket: `/api/cmd/ws/events` (global event stream), `/api/cmd/ws/signal/{spaceId}` (WebRTC signaling relay).
- Event types (`server/event.rs`): SpaceCreated/Deleted/Updated, MemberJoined/Left, MessageSent, FileShared, ScreenShareStarted/Stopped, VoiceCallStarted/Stopped, PeerConnected/Disconnected, ConfigChanged, SystemLog.

### Frontend Events (Desktop)

`new_message` (chat/signaling), `tray-navigate` (tray navigation), `daemon-ready` (daemon ready), `easytier-download-progress` (upgrade progress), `config:changed` (config hot-reload).

### WebRTC Signaling (Key Design)

Voice/screen sharing **signaling control plane reuses the chat message channel**: `msg_type="signal"` carries `SignalEnvelope {kind, type, from, to, data}`, dispatched via `realtime.ts` to `signal.ts`, then routed to `voice.ts` / `screen.ts` (browser-side full-mesh WebRTC, no backend media plane). Deterministic offerer: smaller virtual IP lexicographic order is the offerer.

### Type Sync

| Backend | Frontend | Description |
|---|---|---|
| `src-tauri/src/types.rs` | `src/types/index.ts` | Space / Member / Message / FileInfo parallel types, manually synced |
| `easytier/config.rs` `NetworkConfig` | `src/types/network.ts` | Network config mirror (includes `DEFAULT_NETWORK_CONFIG()`) |

---

## Directory Structure

```
homeTier/
├── src/                        # Frontend React/TS
│   ├── components/             # UI organized by domain (Layout/Space/Chat/Voice/...)
│   ├── stores/                 # Zustand (9 stores: space/settings/file/chat/voice/screen/appTabs/...)
│   ├── services/               # realtime / signal / voice / screen / shortcuts
│   ├── utils/                  # api.ts (dual-mode entry) + api/{tauri,web,core}.ts + utilities
│   ├── i18n/                   # locales (zh / zh-TW / en)
│   ├── types/                  # index.ts (domain models) + network.ts (NetworkConfig)
│   └── hooks/  enum/  styles/
├── src-tauri/                  # Backend Rust
│   ├── src/                    # See "Core Modules" table
│   ├── resources/easytier_lib/easytier   # Built-in EasyTier 2.6.4 (vendored)
│   ├── resources/bin/          # easytier-core fallback binaries
│   ├── tauri.conf.json         # Tauri config (identifier: com.hometier.app, v0.1.0)
│   └── Cargo.toml
├── docs/                       # Design documents (requirements/design/dev/server-mode etc.)
├── deploy/hometier-server.service  # systemd deployment unit
├── Dockerfile                  # Server mode container image
├── homeTier.conf.example       # Config template
└── package.json / pnpm-lock.yaml
```

### Frontend Routes

| Route | Page |
|---|---|
| `/` | Space list (connect/share/config/delete) |
| `/space/:id` | Space home (network stats + app launcher) |
| `/space/:id/chat` `/voice` `/screen` `/files` `/logs` | Chat / Voice / Screen sharing / Files / Logs |
| `/space/:id/app/:appId` | App iframe tab deep link |
| `/settings` | Settings (Basic / EasyTier / Config / Logs tabs) |
| `*` | 404 |

---

## Feature Highlights

- **Space mutual exclusion**: Single EasyTier instance, connecting a new space auto-disconnects the old one, tray menu syncs with language/status.
- **P2P chat**: HMAC-signed messages, optimistic updates + dedup, virtualized message list.
- **WebRTC voice**: Full-mesh direct connect, RMS voice activity detection (150ms sampling, 1.2s silence auto-mute), per-peer volume bars, global shortcuts `Ctrl+M`/`Ctrl+T` (with OSD).
- **Screen sharing**: Invitation-based ACL, quality switching (smooth/standard/hd, maxBitrate control).
- **File transfer**: zstd compression + optional password encryption, streaming with progress, resume progress recovery.
- **App browser**: Embedded iframe accessing any HTTP app within the space (up to 10 LRU tabs), via embedded proxy (hyper) bypassing CSP/X-Frame-Options, injecting fetch/XHR/WebSocket shims to rewrite URLs; desktop/mobile viewport scaling.
- **Distributed config store**: TCP 9788 P2P versioned config sync, anti-version-rollback.
- **Security**: Share links AES-256-GCM encrypted + zstd; chat HMAC verification; files PBKDF2(210k) derived keys; machine ID anti-replay.
- **i18n**: Chinese/Traditional Chinese/English, tray menu hot-syncs with language.

---

## Documentation

| Document | Description |
|---|---|
| [Requirements](docs/需求文档.md) | Product requirements |
| [Design](docs/设计文档.md) | System design |
| [Development](docs/开发文档.md) | Dev environment setup and guide |
| [Server Mode](docs/服务器化改造.md) | Server mode design |
| [Distributed Config Store](docs/分布式配置文件存储服务设计文档.md) | Config storage service design |
| [Third-party App Integration](docs/接入三方应用设计文档.md) | Third-party app integration |

---

## License

This project is licensed under **GPL-3.0**. Dependencies are subject to their respective licenses (EasyTier: Apache-2.0).
