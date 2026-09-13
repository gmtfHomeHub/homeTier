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
- [Platform Support & Known Limitations](#platform-support--known-limitations)
- [CI/CD](#cicd)
- [Development Conventions](#development-conventions)
- [TODO / Roadmap](#todo--roadmap)

---

## Introduction

**homeTier** is a cross-platform virtual LAN application built on the [EasyTier](https://github.com/EasyTier/EasyTier) networking kernel. Desktop targets are Windows / macOS / Linux; Android and iOS are built as Tauri mobile apps. Android VPN connectivity works through the Kotlin `HomeTierVpnService` plugin, while iOS still lacks the host-app → NetworkExtension start bridge and mobile voice/screen media are not wired to the UI yet (see [Platform Support & Known Limitations](#platform-support--known-limitations) and [TODO / Roadmap](#todo--roadmap)). homeTier establishes encrypted P2P virtual networks ("spaces") among devices, on top of which it provides chat, voice, screen sharing, file transfer, LAN app browsing and distributed config storage.

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
| Networking kernel | Built-in EasyTier 2.6.4 (`src-tauri/resources/easytier_lib/easytier`, vendored read-only) |
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
| State management | Zustand 4 (13 stores) |
| Styling | Tailwind CSS 3 + Radix Themes 3 |
| Routing | React Router 6 |
| i18n | react-i18next (zh / zh-TW / en, default zh) |
| Package manager | pnpm 9+ |

> **EasyTier features**: there is no top-level Cargo feature switch for the desktop build. `src-tauri/Cargo.toml` only defines `[features] default = ["custom-protocol"]`; the `wireguard` / `websocket` / `tun` / `socks5` / `kcp` / `quic` / `zstd` features belong to the vendored `easytier` dependency and are enabled **only** for `android`/`ios` targets (mutually exclusive `target.'cfg(...)'` sections). Desktop uses the plain TCP/RPC layer.

> **pnpm is mandatory**: the repo ships `pnpm-workspace.yaml` + `pnpm-lock.yaml`; always install with `pnpm install`. The root `package-lock.json` is stale and must not be used with npm.

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
- **Split-process model**: the GUI runs unprivileged; the networking `daemon` is a separate root/administrator process started by the GUI — `osascript` on macOS, UAC elevation on Windows, `pkexec` on Linux. CLI flags (`src-tauri/src/main.rs:64-139`): `--daemon` (headless daemon), `--server` (single-process axum + embedded daemon), `--elevated` (marks an already-elevated GUI on Windows/Linux).
- **Log-source split**: the log panel switches between `source=gui` and `source=daemon` (`src/components/Log/LogViewer.tsx`). Because the daemon is a separate process, its stdout/stderr are redirected to `{app_data_dir}/daemon.log` (`src-tauri/src/app/daemon.rs:166`) and daemon logs also travel back over IPC.
- **`proxy_cidrs` model**: app-level per-peer `/32` routes are gone. `NetworkConfig::effective_proxy_cidrs` (`src-tauri/src/easytier/config.rs:257-303`) merges `proxy_cidrs` (new) / `proxy_networks` (legacy) and then automatically appends the local virtual network `virtual_ipv4/network_length`; the daemon installs OS routes for those CIDRs.

### Core Modules

| Module | Responsibility |
|---|---|
| `app/` | Tauri lifecycle glue: `setup` (init DB/daemon/proxy/tray), `exit` cleanup, window visibility, elevation flags |
| `commands/` | 115 registered `#[tauri::command]`s across 24 modules (space / network / chat / file / voice / screen / proxy / config / easytier / daemon / log / tray / signal etc.); `src-tauri/src/commands/space.rs` also holds the JNI-only, unregistered `detect_lan_subnets` (116 `#[tauri::command]` attributes in total) |
| `daemon/` | Headless daemon: TCP IPC server, easytier-core lifecycle management, GUI watchdog, graceful shutdown |
| `space/` | Space orchestration: create/join/leave/connect/disconnect, peer discovery, chat/voice/screen/file server lifecycle, encrypted share links |
| `easytier/` | EasyTier manager: RPC-driven `easytier-core` (desktop) or in-process launcher (mobile), TOML config generation, binary download/upgrade |
| `chat/` | P2P chat: per-space HTTP server + broadcast to peers, HMAC-signed messages |
| `voice/` `screen/` | WebRTC voice/screen sharing engine + signaling server (ports 18100+ / 18200) |
| `file/` | P2P file transfer: zstd compression + optional AES encryption, streaming with progress, HTTP file server (19000 + space_id % 1000) |
| `proxy/` | Embedded HTTP proxy (127.0.0.1 random port): CORS/iframe bypass/HTTPS tunnel/URL rewrite/WebSocket tunnel/`__proxy__` local HTTP proxy |
| `server/` | Server mode: axum routes (`/api/cmd/*`, 68 routes including 2 WebSocket), static assets (embedded dist), Cookie auth, TLS, event bus |
| `config_store/` | P2P distributed config store (TCP 9877): versioned files + checksums + dedup write queue |
| `db/` | SQLite persistence (10 tables), auto-migration on startup |
| `log/` | Unified logging system with `log_info!` / `log_warn!` / `log_error!` / `log_debug!` macros |
| `crypto/` | AES-256-GCM + PBKDF2-HMAC-SHA256 (210k iterations), SHA-256, HMAC signing |
| `platform/` | `PlatformAdapter` platform abstraction (config/log directories), machine ID |

### Mode Comparison

| Dimension | Desktop GUI | Daemon | Server |
|---|---|---|---|
| Frontend | Tauri WebView | None | Any browser (axum serves `dist/`) |
| Daemon location | Separate subprocess (elevated on macOS) | Itself | In-process embedded |
| Command interface | `invoke()` 115 commands | TCP IPC | REST `/api/cmd/*` + WS |
| Realtime events | Tauri `listen("new_message")` | — | WS `/api/cmd/ws/events` + `/api/cmd/ws/signal/{space_id}` |
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
pnpm install                     # Install frontend dependencies (pnpm only)
pnpm tauri dev                   # Launch Tauri dev (Vite:1420 + Rust)
pnpm lint                        # ESLint over src/
pnpm lint:fix                    # ESLint with --fix
```

Frontend-only debugging (no native window):

```bash
pnpm dev                         # Vite dev server on http://localhost:1420
```

### Build

```bash
pnpm build                       # Frontend only: tsc --noEmit && vite build
pnpm build:server                # Web/server-mode frontend bundle (same tsc + vite output)
pnpm tauri build                 # Full desktop production build (tsc + vite + Rust release + installer)
```

Per-platform packaging helpers (from `package.json`):

```bash
pnpm tauri:build:macos           # macOS aarch64 dmg
pnpm tauri:build:macos:x86       # macOS x86_64 dmg
pnpm tauri:build:windows         # Windows x86_64 msi
pnpm tauri:build:linux           # Linux x86_64 deb
pnpm tauri:build:linux:arm       # Linux aarch64 deb
pnpm tauri:build:appimage        # Linux AppImage
pnpm tauri:build:android         # Android APK (arm64 / armv7 / x86_64)
pnpm tauri:build:docker          # Production build without bundling (used by the Docker image)
```

Mobile build helpers live in `scripts/`: `fix-android-mainactivity.sh`, `fix-android-build-gradle.sh`, `mobile-permissions.sh`, `verify-android-injection.sh`, `download-npcap-dlls.sh`.

### Frontend Public Base

`VITE_PUBLIC_BASE` (`.env.example:7`, consumed by `vite.config.ts:5-13`) sets Vite's `base` for subpath / reverse-proxy deployments and must match `SERVER_PUBLIC_BASE` in server mode. **It must stay `/` for every Tauri (desktop/mobile) build**, because Tauri loads `dist/` from the root path.

### Backend Type Check

```bash
cd src-tauri && cargo check --all-targets   # Same check CI runs (requires local cc linker)
```

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

> `SERVER_BIND` / `SERVER_PORT` / `SERVER_STATIC_DIR` are runtime-generated defaults (`0.0.0.0` / `9339` / `./dist`). They are **not** keys in `homeTier.conf.example`, which only ships `DAEMON_IPC_PORT`, `EASYTIER_RPC_PORT`, `FILE_SERVER_PORT_BASE`, `DEFAULT_SPACE_IP`, `GITHUB_API`, `GITHUB_MIRROR`, `RELAY_NETWORK_PREFIX`, `LOG_ENABLED`.

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
pnpm build:server                 # Produce dist/ before copying (server mode serves the web bundle)
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

SQLite file at `{app_data_dir}/homeTier.db` (server mode: `{server-dir}/homeTier.db`), auto-migrated on startup (`src-tauri/src/db/migrations.rs`), 10 tables:

| Table | Description |
|---|---|
| `users` | Local users (machine identity) |
| `spaces` | Spaces (includes `network_secret`, `config_json`) |
| `members` | Members (virtual IP, online status, owner flag) |
| `messages` | Chat messages (includes send status) |
| `files` | File records |
| `settings` | Key-value settings |
| `proxy_cookies` | Cookie jar for the embedded app browser/proxy |
| `space_apps` | In-space apps (for iframe browser) |
| `acl_rules` | ACL rules |
| `port_forward_rules` | Port forwarding rules |

### Port Conventions

| Port | Usage |
|---|---|
| `15889` | daemon TCP IPC (configurable) |
| `15888` | easytier-core RPC (configurable) |
| `9877` | Distributed config store TCP (P2P) |
| `9339` | Server mode HTTP (configurable) |
| `19000 + space_id % 1000` | File transfer HTTP server |
| `18100 + space_id % 1000` | Voice signaling server |
| `18200 + space_id % 1000` | Screen sharing signaling server |
| `127.0.0.1:<random>` | Embedded HTTP/HTTPS/WSS proxy (ephemeral per-process port) |

### Share Links

Format: `homeTier://join?v=1&d={base64url}`. Payload flow: ShareInfo **binary-encoded (little-endian, 1-byte length prefixes, optional-field bitmask)** → **adaptive compress (zstd level 3; falls back to raw when compressed ≥ original)** → **AES-256-GCM encrypt** (key = SHA-256(config `SHARE_LINK_SECRET`, default `homeTier-qr-v1`)) → base64url (no padding). Chat messages use the space `network_secret` for HMAC-SHA256 signature verification; password-protected files use PBKDF2-derived key encryption.

---

## Frontend–Backend Contract

### Tauri Commands (Desktop)

- **115** registered `#[tauri::command]`s across 24 modules in `src-tauri/src/commands/`, registered in `src-tauri/src/lib.rs:53-198`. A 116th command attribute, `detect_lan_subnets` (`src-tauri/src/commands/space.rs`), is JNI-only and intentionally not registered.
- Frontend unified wrapper at `src/utils/api.ts:14`: runtime detection of `__TAURI_INTERNALS__` selects the **Tauri `invoke()`** implementation (`src/utils/api/tauri.ts`, ~70 distinct `invoke(...)` targets) or the **REST/WS** implementation (`src/utils/api/web.ts`) — business code is agnostic.
- Main command domains: space (15), mobile_voice (9), config_store (8), mobile_screen (8), app (8), log (7), file/proxy (6 each), daemon/util (5 each), config/easytier/network/ACL/port-forward/voice/screen (4 each), ios_vpn/chat/update_app (2 each), qr/signal/tray/mobile_vpn (1 each).

### Server Mode REST + WS

- REST: `/api/cmd/*` (68 routes: ping / space / chat / network / log / config / file / proxy / easytier / config-store etc.), JSON + cookie auth.
- WebSocket: `/api/cmd/ws/events` (global event stream), `/api/cmd/ws/signal/{space_id}` (WebRTC signaling relay).
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
│   ├── stores/                 # Zustand (13 stores: space/settings/file/chat/voice/screen/layout/peer/update/...)
│   ├── services/               # realtime / signal / voice / screen / mobileVpn / shortcuts
│   ├── utils/                  # api.ts (dual-mode entry) + api/{tauri,web,core}.ts + utilities
│   ├── i18n/                   # locales (zh / zh-TW / en)
│   ├── types/                  # index.ts (domain models) + network.ts (NetworkConfig)
│   └── hooks/  enum/  styles/
├── src-tauri/                  # Backend Rust
│   ├── src/                    # See "Core Modules" table (commands/: 24 modules, 115 registered handlers)
│   ├── resources/easytier_lib/easytier   # Built-in EasyTier 2.6.4 (vendored, read-only)
│   ├── resources/bin/          # easytier-core fallback binaries + Windows DLLs (wintun/WinDivert/wpcap)
│   ├── tauri.conf.json         # Tauri config (identifier: com.hometier.app, v0.1.0)
│   └── Cargo.toml
├── deploy/hometier-server.service  # systemd deployment unit
├── Dockerfile                  # Server mode container image
├── homeTier.conf.example       # Config template
└── package.json / pnpm-lock.yaml
```

> `src-tauri/resources/easytier_lib/` is a vendored, read-only copy of the EasyTier upstream tree. Do not edit it (see [Development Conventions](#development-conventions)).

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

- **Space networking**: create / join / leave / delete spaces, connect / disconnect with mutual exclusion (one EasyTier instance at a time), encrypted `homeTier://join?...` share links and QR codes.
- **Networking view**: members, mesh routes, traffic statistics, ACL rules and port-forwarding rules.
- **Signed P2P chat**: HMAC-SHA256 signed messages, optimistic updates + dedup, virtualized list, `new_message` events; signaling rides the same channel.
- **P2P file transfer**: zstd compression + optional password encryption, streaming with progress and SHA-256 verification.
- **WebRTC voice (desktop)**: full-mesh direct connect, RMS voice-activity detection (150ms sampling, 1.2s silence auto-mute), per-peer volume bars, global shortcuts `Ctrl+M` / `Ctrl+T` (with OSD). Implemented in the frontend (`src/services/voice.ts`), signaling over the chat channel.
- **Screen sharing (desktop)**: invitation-based ACL and three quality tiers (smooth / standard / hd), implemented in `src/services/screen.ts`.
- **App browser + proxy**: embedded iframe with up to 10 LRU tabs and a built-in HTTP/HTTPS/WSS reverse proxy (self-signed CA trust, iframe/X-Frame-Options bypass, cookie jar, fetch/XHR/WebSocket URL rewriting).
- **Dual-source logging**: switch between GUI logs and daemon logs (`source=gui` / `source=daemon`) in the log viewer, with module filters and export.
- **Config center**: hot-reloaded `homeTier.conf` plus a P2P distributed config store (TCP 9877) with versioned files, checksums and anti-rollback.
- **EasyTier version management**: check / download / upgrade the bundled `easytier-core` with progress; app self-update via the project's own GitHub Releases (no Tauri updater).
- **Tray & background**: system tray menu, background running, single-instance handling.
- **Three run modes**: desktop GUI (`homeTier`), headless daemon (`--daemon`), single-process server (`--server`), plus mobile builds.
- **i18n**: Simplified Chinese (default), Traditional Chinese and English; tray menu hot-syncs with language.
- **Security**: share links AES-256-GCM + zstd; chat HMAC verification; files PBKDF2(210k) derived keys; machine ID anti-replay.

---

## Platform Support & Known Limitations

| Platform | Support |
|---|---|
| Windows | Full desktop support + UAC elevation. Bundles Npcap (`wpcap.dll`), WinDivert (`WinDivert64.sys`, `packet.dll`) and Wintun (`wintun.dll`) in `src-tauri/resources/bin/`. |
| macOS | Full desktop support. The GUI stays unprivileged; the daemon is elevated via `osascript`. `pkexec` is used on Linux. |
| Linux | Full desktop support, `.deb` and AppImage packages. |
| Android | APK builds (arm64 / armv7 / x86_64). VPN is provided by the Kotlin `HomeTierVpnService` plugin (`src-tauri/scripts/android/`). Voice and screen native bridges exist but are not wired to the UI, and `get_vpn_status` is a placeholder (`src-tauri/src/commands/mobile_vpn.rs:16`). |
| iOS | A NetworkExtension static library and an Xcode injection script exist, but there is no host-app → NE start bridge and media (voice / screen) is TODO. VPN never comes up. |

Notes:

- **Platform adapters are thin**: `PlatformAdapter` implementations only resolve config/log directories and the machine ID (`src-tauri/src/platform/mod.rs:19`); all networking and process code is shared.
- **No official Tauri updater**: the app self-updates by checking its own GitHub Releases (`src-tauri/src/commands/update_app.rs`).
- **Mobile VPN interface IP**: The default IP address of the VPN interface for Android/iOS is equal to the IP address of the EasyTier node, and it does not support using `10.144.144.1` (with `.10` as the fallback for mobile devices).

---

## CI/CD

- **`.github/workflows/ci.yml`** — runs on every push / PR with two jobs:
  - `frontend`: pnpm 9 + Node 22, `pnpm install --frozen-lockfile`, then `pnpm lint` and `pnpm build` (tsc + vite).
  - `backend`: Rust stable + `rust-cache`, installs webkit2gtk/GTK system deps, runs `mkdir -p dist` (the compiler macros require the dir to exist), then `cargo check --all-targets` in `src-tauri`.
  - There is **no test job**: CI runs no `cargo test` / `pnpm test` (see the P3 item in [TODO / Roadmap](#todo--roadmap)).
- **`.github/workflows/release.yml`** — triggered manually (`workflow_dispatch`, with per-platform toggles) or by `v*` tags. First `fetch-easytier` downloads per-platform `easytier-core` archives, then builds macOS dmg (aarch64 + x86_64), Windows msi (x86_64; ARM64 currently paused), Linux deb (x86_64 + aarch64), Linux AppImage, Android apk, and a Docker image pushed to GHCR. iOS is skipped by default (the `未完成平台隔离，默认跳过` input gate).
- **`.github/actions/setup-build`** — composite action that installs pnpm / Node / Rust, restores caches, optionally downloads the EasyTier artifact, installs Linux deps and runs `pnpm install --frozen-lockfile`. It deliberately does not check out the repo (callers do).

---

## Development Conventions

- **pnpm only**: the repo uses `pnpm-workspace.yaml` + `pnpm-lock.yaml`; never install with npm/yarn (`package-lock.json` is stale).
- Run `pnpm lint` (or `pnpm lint:fix`) before committing.
- **Never edit `src-tauri/resources/easytier_lib/`** — it is a vendored, read-only copy of upstream EasyTier. Only `edition` / `rust-version` metadata may be touched if strictly required.
- Custom `log_info!` / `log_warn!` / `log_error!` / `log_debug!` macros write into the in-memory log store (and forwarding targets), **not** to stdout.
- The daemon and the GUI are separate processes with separate logs; use the log viewer's `source` switch (`source=gui` / `source=daemon`) instead of assuming a single stream.
- Use the shared `Tip` wrapper (`src/components/Common/Tip.tsx`) instead of raw Radix `Tooltip`.
- `patch_config` is now safe for runtime config changes (fixed in `src-tauri/src/easytier/mod.rs` with JSON-backed full config serialization).

---

## TODO / Roadmap

| Priority | Item | Evidence / Impact | Size |
|---|---|---|---|
| P0 | iOS system VPN start bridge (host app → NetworkExtension) | `src-tauri/src/commands/ios_vpn.rs:69-72` only emits `ios:start-vpn` with no receiver; `src/services/mobileVpn.ts:143` always calls the Android-only `plugin:hometiervpnservice\|start_vpn`; `src-tauri/gen-scripts/ios/` contains only NE-extension files, no host `@main`/AppDelegate/`NETunnelProviderManager.startVPNTunnel` caller. Impact: VPN never comes up on iOS. Depends on Xcode project + Apple Developer account + device. | L |
| P1 | Mobile voice calling | `src-tauri/src/voice/mobile/android.rs:75-160` JNI targets a Kotlin `VoiceManager` that does not exist in the repo; `src-tauri/src/voice/mobile/ios.rs` is all TODO; `src/stores/mobileVoiceStore.ts:46-49` has the `invoke` calls commented out. | L |
| P1 | Mobile screen sharing | `src-tauri/scripts/android/screen/ScreenShareManager.kt:90-104` creates the VirtualDisplay with `Surface = null` (no capture); the frame callback in `src-tauri/src/screen/mobile/android.rs:262-275` is still a placeholder (`后续实现`); `src-tauri/src/screen/mobile.rs:192` TODO ReplayKit; `src/stores/mobileScreenStore.ts:31-46` invoke calls commented out. | L |
| P2 | Mobile easytier-core update path is a stub | `src-tauri/src/commands/mobile_vpn.rs:16` placeholder `get_vpn_status`; update UI hidden on mobile (`src/components/Settings/EasyTierVersionManager.tsx:114`). | M |
| P2 | Windows ARM64 desktop bundle missing from the release matrix | `docs/workflow.md:298`. | M |
| P2 | macOS notarization + Windows code signing | `docs/workflow.md:136` and `:254-271` (deferred). | L |
| P2 | iOS NE signing / App Store compliance | KVC private-API risk for the TUN fd: `docs/mobile_vpn.md:1152-1159`. | L |
| P2 | Server mode: P2P transfer progress not implemented | `src-tauri/src/server/routes.rs:1566` returns NOT_IMPLEMENTED. | S |
| P2 | stock EasyTier → homeTier cross-/24 needs a manual `proxy_cidr` on the stock side | Native EasyTier behavior; needs a user-facing FAQ. `src-tauri/src/easytier/config.rs:257-303`. | S |
| P3 | CI runs no tests | `.github/workflows/ci.yml:12-60` only `pnpm lint` / `pnpm build` + `cargo check --all-targets`, while the repo already has ~35 Rust `#[test]` / `#[tokio::test]` functions. | M |

Items already done are intentionally absent. Upstream TODOs inside `easytier_lib/` are excluded from this list.

---

## License

This project is licensed under **GPL-3.0-or-later**. See the `LICENSE` file at the repository root (the same text also exists as `GPL-3.0 license`). Dependencies are subject to their respective licenses (EasyTier: Apache-2.0).
