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

## ⛔ Forbidden: easytier_lib read-only
**绝对不允许编辑、修改、创建或删除 `src-tauri/resources/easytier_lib/` 下的任何文件。** 该目录是 vendored 的第三方 EasyTier 库源码，必须保持与上游一致。如果发现编译错误（如 edition 版本不匹配），只允许修改其 `Cargo.toml` 中的 `edition` / `rust-version` 字段以匹配上游要求，不允许改动任何 `.rs` 源文件、`build.rs`、或其他配置。遇到 easytier_lib 相关编译问题时，应先查上游仓库确认正确配置。

# AGENTS.md

本文档用于约束本项目中的 AI / 自动化开发行为。开发时优先遵循本文件，其次遵循用户当前消息。

## 基本原则

- 先读现有代码，再动手修改，优先沿用项目已有结构和写法。
- 写代码保持最少行数，能简单实现就不要引入复杂抽象。
- 标准格式、协议、解析、压缩、加密、日期等通用能力优先使用成熟稳定的库，不要手写底层实现，除非用户明确要求或项目已有实现必须沿用。
- 不要为了“兼容更多场景”写大量分支，只实现当前明确需要的功能。
- 禁止用新增固定延时、轮询次数、重试次数、输出上限或其他拍脑袋常数掩盖状态与性能缺陷。限制只能来自供应商公开约束、管理员配置或项目已有且有测试依据的资源保护契约；没有依据时保持上游与现有配置语义，不自行降级性能。
- 项目尚未上线，不需要兼容旧数据；表结构或字段调整时直接按新设计修改，不写旧字段兼容、数据迁移兜底或删除旧表的清理逻辑，除非用户明确要求。
- 每次写完代码必须运行与改动相关的测试和类型检查；任务收尾按“Mandatory Testing”执行全量质量门禁与浏览器回归。
- 不要改无关文件，不要顺手重构。
- 如果工作区已有用户改动，不要回滚，不要覆盖；只在必要范围内追加修改。
- 含中文的源码、配置、脚本和文档统一保存为 UTF-8；PowerShell 读取时显式使用 `-Encoding UTF8`。发布验收必须严格解码文本文件，并检查 `�`、`锟斤拷` 等常见乱码标记，不能把终端显示异常直接当成文件损坏。

## 反复提醒沉淀

- 如果开发过程中总是遇到某个问题，或者用户反复提醒同一个注意事项，需要把该注意事项补充到本文件。
- 补充时写成明确、可执行的规则，避免只写模糊描述。
- 新规则应放到最相关的章节；找不到合适章节时放到“项目注意事项”。
