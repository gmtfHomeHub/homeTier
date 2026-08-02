# homeTier — Agent Guide

## Stack
- **Desktop/mobile**: Tauri 2.x (Rust backend + React/TS frontend via Vite 5)
- **Package manager**: pnpm 9+ (not npm)
- **Frontend**: React 18, TypeScript 5.5 strict, Zustand, Tailwind 3 + Radix Themes, react-i18next (default locale `zh`), React Router 6, `@/*` → `./src/*`
- **Backend**: Rust 2021 edition with Tokio, vendored EasyTier at `third_libs/easytier/` (Rust 2024 ed., MSRV 1.95)
- **No linter or formatter** is configured. Only quality gate is `tsc --noEmit` in `pnpm build`.

## Key commands
| Command | What it does |
|---|---|
| `pnpm dev` | Vite dev server on port 1420 |
| `pnpm build` | `tsc --noEmit && vite build` (typecheck then bundle) |
| `pnpm tauri dev` | Launch Tauri dev (native window + Vite) |
| `pnpm tauri build` | Production build (frontend + Rust, platform bundle) |
| `cargo check` | Backend type-check only (from `src-tauri/`) — preferred over `cargo build` |
| `cargo build` | Full backend compilation (from `src-tauri/`) |
| `codegraph index` | Run after each commit to keep CodeGraph index up to date |

Vite ignores `**/src-tauri/**` — no hot-reload on Rust changes.

## Code understanding
CodeGraph is indexed at `.codegraph/`. Prefer `codegraph explore "<query>"` over grep for understanding symbols, call paths, and source locations.

## Architecture essentials
- **Spaces are mutually exclusive**: only one EasyTier network instance runs at a time. `SpaceManager::connect` disconnects any current connection.
- **151 Tauri `#[tauri::command]` handlers** registered in `src-tauri/src/lib.rs`. Frontend calls them via `invoke()` in `src/utils/api.ts`; backend pushes events via `listen()`.
- **Custom log macros** (`log_info!`, `log_error!`, `log_warn!`, `log_debug!`) defined in `src-tauri/src/log/mod.rs` — writes to in-memory store, not stdout/stderr. Optional second param is `space_id` for filtering.
- **HTTP proxy** runs internally on `127.0.0.1:<random-port>` (hyper 1.x) to bypass CSP/X-Frame-Options for iframe content. CSP is `null` in Tauri config.
- **Database** is SQLite (rusqlite bundled) at `{app_data_dir}/homeTier.db`, auto-migrated on startup. Schema in `src-tauri/src/db/migrations.rs`.
- **Tauri plugins**: shell, process, clipboard-manager, global-shortcut, os, single-instance.
- **Window hides to tray** on close (does not quit). Handled via `CloseRequested` with `prevent_close()`.
- **Mobile support**: viewport disables pinch-zoom (`maximum-scale=1`), safe-area padding on `body`, fixed-width dialogs use `w-full max-w-[calc(100vw-24px)] sm:w-[原值]`, tables wrap in `overflow-x-auto` (BaseTable does this). **Tooltip rule**: never wrap clickable controls in `Tooltip` — on touch it becomes two-step interaction (first tap shows tooltip, second fires action); use `toastInfo`/`toastError` from `src/utils/toast.ts` instead. Tooltip is only for non-interactive info icons.
- **EasyTier compile features** required: `wireguard`, `websocket`, `tun`, `socks5`, `kcp`, `quic`, `zstd`.

## Type sync
Frontend (`src/types/index.ts`) and backend (`src-tauri/src/types.rs`) define parallel types (Space, Member, Message, etc.). Keep them in sync manually. `src/types/config.ts` mirrors the Rust `NetworkConfig` struct.

## Platform notes
- Platform abstraction via `PlatformAdapter` trait in `src-tauri/src/platform/`, with `#[cfg]`-gated impls for windows, macos, android, ios.
- Android/iOS stubs exist; voice and screen-share have placeholder implementations and may be incomplete.
- `tauri.conf.json` identifier: `com.hometier.app`, version `0.1.0`, window 1000×700 (min 800×600).
