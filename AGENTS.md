# homeTier — Agent Guide

## Stack
- **Desktop/mobile**: Tauri 2.x (Rust backend + React/TS frontend via Vite 5)
- **Package manager**: pnpm 9+ (not npm)
- **Frontend**: React 18, TypeScript 5.5 strict, Zustand, Tailwind 3 + Radix Themes, react-i18next (default locale `zh`), React Router 6, `@/*` → `./src/*`
- **Backend**: Rust 2021 edition with Tokio, vendored EasyTier at `src-tauri/resources/easytier_lib/easytier` (Rust 2024 ed., MSRV 1.95)
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
- **Mobile support**: viewport disables pinch-zoom (`maximum-scale=1`), safe-area padding on `body`, fixed-width dialogs use `w-full max-w-[calc(100vw-24px)] sm:w-[原值]`, tables wrap in `overflow-x-auto` (BaseTable does this). **Tooltip rule**: never wrap clickable controls in `Tooltip` — on touch it becomes two-step interaction (first tap shows tooltip, second fires action); use `toastInfo`/`toastError` from `src/utils/toast.ts` instead. Tooltip is only for non-interactive info icons. **Always use the `Tip` wrapper (`src/components/Common/Tip.tsx`) instead of importing Radix `Tooltip` directly** — it keeps desktop hover behavior and degrades to tap-toast (string content) / long-press popover (rich content) on touch devices; direct `Tooltip` imports are forbidden.
- **EasyTier compile features** required: `wireguard`, `websocket`, `tun`, `socks5`, `kcp`, `quic`, `zstd`.

## Type sync
Frontend (`src/types/index.ts`) and backend (`src-tauri/src/types.rs`) define parallel types (Space, Member, Message, etc.). Keep them in sync manually. `src/types/config.ts` mirrors the Rust `NetworkConfig` struct.

## Platform notes
- Platform abstraction via `PlatformAdapter` trait in `src-tauri/src/platform/`, with `#[cfg]`-gated impls for windows, macos, android, ios.
- Android/iOS stubs exist; voice and screen-share have placeholder implementations and may be incomplete.
- `tauri.conf.json` identifier: `com.hometier.app`, version `0.1.0`, window 1000×700 (min 800×600).

## 项目注意事项

- **GUI 与 daemon 是两个独立进程，日志互不可见**：应用内日志面板默认只含 GUI 进程日志；daemon（root）侧日志要用面板切到 **source=daemon**（走 IPC `GetLogs`），或直接看 root 进程 stdout 被重定向的 `~/Library/Application Support/com.hometier.app/daemon.log`。验证 daemon 侧改动（如 `daemon/peer_routes.rs`、`build_proto_config`）必须看这两处，只看应用日志会误判“代码没跑”。同时注意 daemon 由 GUI 的 `current_exe` 经 osascript/UAC 提权拉起 —— 改完 Rust 后必须确认重新编译并新拉起 daemon（旧进程仍占默认 IPC 端口时会换端口或连到旧进程）。
- **跨 /24 虚拟 IP 互通靠 easytier 原生 proxy_cidrs 模型，禁用应用层对端 /32**：原 `daemon/peer_routes.rs`（每对端 /32 OS 路由）已废弃删除——它要求两端相互加路由，而原生 EasyTier 从不加，导致非对称回程超时。新模型：本机经 `config.rs::effective_proxy_cidrs` 自动把本机虚拟 /24（如 `10.144.144.0/24`）注入 proxy_cidrs 宣告给对端；对端收到的 proxy_cidrs 由 easytier 内部自动收敛——桌面端 easytier-core 子进程 `run_proxy_cidrs_route_updater` 自动加 OS 路由，移动端 `mesh_routes_updated` 事件 → `rebuildVpn` 重建 VpnService 路由。跨 /24 在 homeTier↔homeTier 间全自动；homeTier→stock 单向自动；stock→homeTier 跨 /24 需 stock 侧手动配 proxy_cidr（easytier 原生不自动宣告自己虚拟 /24，非 homeTier 缺陷）。同 /24（统一 10.144.144.0/24）与 stock 全自动直连。
- **移动端 VpnService 接口 IP 必须等于 EasyTier 节点身份 IP**：两者不一致会导致 mesh L3 回包黑洞（本机回包从物理网卡/默认路由被物理邻居 ARP 劫走）。Android VpnService 必须在实例建立前定死接口地址，不支持“DHCP 先建 tun 后定址”，移动端一律走静态 IP；分配 IP 时避开 `10.144.144.1`（空间默认网段 10.144.144.0/24 内多节点易冲突，VpnService 兜底默认已改为 `.10`，前端兜底禁止用 `.1`）。
- **`EasyTierManager::patch_config` 有丢字段 bug，禁止用于改运行配置**：`read_network_config` 只解析 TOML `[network_identity]` 的 network_name/network_secret，其余字段（dhcp/ipv4/listeners/networking_method/public_server_url/peers 等）全部回落 `NetworkConfig::default()`；`patch_config` 的 if 链无 `proxy_cidrs` 分支（patch 该字段被静默忽略）；末尾 `generate_config` 用残缺 config **覆盖原 TOML** + `rpc_run_network_instance` 重启 → 必断网。daemon 侧自动 mesh_routes→proxy_cidrs patch 任务已移除（冗余 + 语义错误：把对端子网当本机宣告）。`patch_config` 方法与 `PatchSpaceConfig` IPC handler 保留但当前无前端调用。未来若需运行时改配置，必须先让 `read_network_config` 完整解析 TOML（或改用 serde + `config::NetworkConfig` 的 Serialize/Deserialize）。
- **Android per-ABI 构建只能用 Tauri CLI 原生 flag，禁止手写 Gradle ABI 配置**：`RustPlugin.kt`（由 `@tauri-apps/cli` 生成，模板内嵌在 CLI 二进制里）用生成期硬编码的 `defaultAbiList/defaultArchList`（含 `x86`）建 ABI product flavor，用构建期属性 `-PabiList=/-ParchList=/-PtargetList=`（即 CLI `--target`）接线 `rustBuild<Arch><Profile>` 任务，两者以 `targetPair.index` 交叉索引。由此得出四条硬规则：(1) 默认构建会创建 `rustBuildX86Release` 去编 `i686-linux-android`，而 CI 实际用的 NDK 29 已移除 32 位 x86 → `kcp-sys` 的 bindgen 报 `bits/libc-header-start.h file not found`；(2) **不要**改 `defaultArchList`/`defaultAbiList` 去删 `x86`——`archList` 仍是构建期值，下标会越过 flavor 列表导致 `tasks["mergeX86…JniLibFolders"]` 找不到，配置期直接崩；(3) **不要**向 `app/build.gradle.kts` 注入 `ndk.abiFilters` 或 `splits.abi`——与模板的 ABI flavor 互斥，报 `Conflicting configuration … cannot be present when splits abi filters are set`；(4) 补 `src-tauri/gen/android/tauri.build.gradle.kts`（根目录那个）是**无效的**，真实文件在 `app/` 下且不含 ABI 列表——历史上 5 次失败修复都打在了这个不存在的文件上。正确做法：`pnpm tauri android build --apk --target aarch64 armv7 x86_64 --split-per-abi`，再用 APK 内 `lib/<abi>/` 的实际内容筛选产物（丢弃 `universal` 与无 native 库的空 `x86` APK）。
- **`android-actions/setup-android` 的 `packages` 默认值会让 Android CI 直接失败，且该 action 根本没有 `ndk` 输入**：该 action 把 `packages` 按空格 split 后**对每个包单独调一次 `sdkmanager`**，而默认值 `'tools platform-tools'` 里的 `tools`（旧 SDK Tools）早已被 Google 从 SDK 仓库下架（`repository2-1/2/3.xml` 里 `name="tools"` 命中 0 个，`platform-tools` 仍在）；该 action 默认 `cmdline-tools-version: 12266719`，经内部 `getVersionShort()` 映射成 `16.0`，恰好命中 runner 镜像预装的 cmdline-tools 16.0，而它的 sdkmanager 把「包不存在」当致命错误（exit 1），`@actions/exec` 默认 `ignoreReturnCode=false` → 步骤抛错终止。表现：`Warning: Failed to find package 'tools'` + `sdkmanager failed with exit code 1`。**升级 action 无效**——最新 v4.0.1 仍带同样的默认值。修法：显式写 `packages: platform-tools`。另外该 action 的输入只有 `cmdline-tools-version` / `accept-android-sdk-licenses` / `log-accepted-android-sdk-licenses` / `packages` 四个，**没有 `ndk`**——之前 `with: ndk: 25.2.9519653` 一直是静默无效的多余参数，构建实际一直用 runner 镜像预装的 NDK（这就是历史上「声明 25.x 实际 29.x」漂移的真正原因）。现由 `Pin Android NDK` 步骤显式 `sdkmanager "ndk;29.0.14206865"` 并把 `ANDROID_NDK_HOME`/`NDK_HOME` 写入 `$GITHUB_ENV`；该版本已确认在 Google SDK 仓库可下载，且与之前实际使用的版本一致（零行为变化），镜像不再预装时也会自动补装。
- **产物命名规范 = `{productName}-{os}-{arch}-{version}{suffix}.{ext}`，版本号放末尾**：桌面 os ∈ `linux`/`macos`/`windows`，arch 取 Rust target 前缀（`x86_64`/`aarch64`）；Android arch 用 **ABI 原名**（`arm64-v8a`/`armeabi-v7a`/`x86_64`）；iOS 固定 `homeTier-ios-arm64-<ver>.zip`；`-debug` 仅在 `inputs.debug_mode=true` 时追加。桌面/AppImage 由 `scripts/normalize-artifact-name.sh` 改名，Android/iOS 由 workflow 内联 `DEST` 实现；版本号统一从 `tauri.conf.json` 读（`productName`/`version`），release job 有 tag 与 conf version 一致性断言。此格式对齐上游 EasyTier 的 `easytier-{os}-{arch}-{version}.zip` —— 注意上游**其实只对 Rust core 的 zip 改名**，`.dmg`/`.msi`/`.deb`/`.AppImage`/`.apk` 全部保留 Tauri 原始名（`release.yml` 里 `find release_assets_nozip -type f -exec mv {}` 原样搬走），我们是把这套格式扩展到安装包。版本号放末尾对 Homebrew Cask（手写 cask 的 `url` 是字面量）/Chocolatey（版本在 `.nuspec`）/Scoop/WinGet（manifest + sha256）/`msiexec`（读内部 `ProductVersion`）/`dpkg`/`rpm`（读内部 control 元数据）/`pm install`（读内部 `AndroidManifest.xml`）**均无功能影响**，唯一硬约束是**未来若自建 apt 仓库**需回退到 `{pkg}_{ver}~{arch}.deb`。
- **改产物文件名必须同步三处，否则静默失效**：① `src-tauri/src/commands/update_app.rs::appimage_asset_keyword()` —— Linux 应用内更新按资产名**前缀**（`homeTier-linux-x86_64-`）+ `.AppImage` 后缀在 GitHub Release 资产里查找；版本号在文件名末尾且每次发布都变，**不能用 `ends_with` 匹配**。历史坑：该函数曾硬编码 `_amd64.AppImage`（匹配 Tauri 原始名 `homeTier_0.1.0_amd64.AppImage`），产物改名后既不匹配新名也不匹配任何已发布资产 → AppImage 永不更新。② workflow 的 `download-artifact` pattern 必须是 `homeTier-*`，且每个 `upload-artifact` 容器名必须以 `homeTier-` 开头（否则不进 GitHub Release）；历史上 `homeTier.AppImage`（无连字符）因此从未被发布过。③ release job 的收集 glob 需随格式同步（现为 `homeTier-ios-*.zip`）。
- **`easytier-{os}-{arch}-{version}.zip` 是上游锁定契约，禁止改名**：`fetch-easytier` 的下载 URL 硬编码为上游 EasyTier Release 资产名（`easytier-${os}-${arch}-${EASYTIER_CORE_VERSION}.zip`），且 sha256 校验按**精确文件名**在上游 Release API 的 `.assets[]` 里查找（查不到即 `CHECKSUM MISMATCH` 失败）。配套的本地 artifact 容器名 `easytier-bin-*`（11 处）一并不可动。改 homeTier 自身产物名时不得顺手改这两处。
- **iOS VPN 启动链路是既有架构 stub，NE extension 已实现但 host app 无桥接启动**：`gen-scripts/ios/` 只有 NE extension 文件（PacketTunnelProvider/TunnelHelper/BuilderHelper），无 host app 入口（无 AppDelegate/`@main`/`NETunnelProviderManager.startVPNTunnel` 调用）。`mobileVpn.ts` 走 Android 插件 `plugin:hometiervpnservice|start_vpn`（iOS 无对应）。`start_ios_vpn` 只写 App Group 配置 + emit `ios:start-vpn` 事件（无 Swift bridge 监听来调 `startVPNTunnel`）。结果：iOS 上 VPN 无法启动，S4 加的 `PacketTunnelProvider.handleAppMessage("rebuild_routes")` 接收侧无发送方也无运行中的 extension 可接收。完整 iOS 集成需：(1) host app Swift bridge 调 `NETunnelProviderManager.loadAllFromPreferences` + `startVPNTunnel`；(2) `mobileVpn.ts` 加 iOS 分支调 `start_ios_vpn` 而非 Android 插件；(3) host→extension 路由重建触发（`sendProviderMessage` app message，或 Darwin notify：`PacketTunnelProvider` 启动时 `registerDarwinNotify("com.hometier.routeRebuild", applyNetworkSettings)` + Rust FFI `notify_post`）。此为 iOS 原生集成专项，需 Xcode + 真机，非路由逻辑问题。

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
