# GitHub Workflow 配置指南（从 0 开始）

本文档面向从未接触过 GitHub Actions 的开发者，完整说明 homeTier 的 CI/CD 工作流如何从零配置、运行与运维。

> 适用版本：仓库 `.github/workflows/` 当前包含的 `release.yml`（发布打包）与 `ci.yml`（日常检查）。
> 最近一次修订：补齐「从 0 开始」全流程，修正移动端权限注入路径、增加归档校验与构建缓存。

---

## 1. 总览

| 文件 | 触发方式 | 功能 |
|---|---|---|
| `.github/workflows/release.yml` | 手动触发（Actions 页）或推送 `v*` 标签 | 下载 easytier-core → 7 大平台打包 → GHCR 镜像 → GitHub Release |
| `.github/workflows/ci.yml` | push / pull_request | 前端 lint+构建、后端 `cargo check`，防止坏代码合入 |

### release.yml 流水线（7 个 job）

```
fetch-easytier ──┬─► build-desktop（5 平台矩阵: mac dmg×2 / win msi / linux deb×2）
                 ├─► build-appimage（Linux AppImage）
                 ├─► build-android（APK，3 种 ABI）
                 ├─► build-ios（无签名 .app）
                 └─► build-docker（GHCR 镜像）

各 build job 产物 ──► release（仅 tag 触发时汇总发布）
```

---

## 2. 从 0 开始配置

### 2.1 前置条件

1. **GitHub 仓库**：代码已推送到 GitHub（本项目远端为 `git@github.com:gmtfHomeHub/homeTier.git`）。
2. **Actions 开关**（只用于禁用过的情况）：
   - 仓库页 → `Settings` → `Actions` → `General` → 确认 **Allow all actions and reusable workflows** 已开启。
3. **网络**：GitHub Hosted Runner 本身在 GitHub 网络内，可直连 github.com 下载依赖（无需代理）。

### 2.2 创建目录与文件

```bash
mkdir -p .github/workflows
```

把 `release.yml`、`ci.yml` 放入该目录并提交。**.github/workflows/** 下的 `.yml` 文件推送后即被 GitHub 识别，无需任何注册步骤。

### 2.3 依赖的 Action（GitHub 市场，无需手动安装）

| Action | 用途 |
|---|---|
| `actions/checkout@v4` | 拉取仓库代码 |
| `actions/setup-node@v4` | 安装 Node.js（带 pnpm 缓存） |
| `pnpm/action-setup@v4` | 安装 pnpm 9 |
| `dtolnay/rust-toolchain@stable` | 安装 Rust stable + 交叉编译 target |
| `swatinem/rust-cache@v2` | 缓存 cargo 编译产物（`workspaces: src-tauri -> target`）|
| `actions/download-artifact@v4` / `upload-artifact@v4` | job 间传递构建产物 |
| `actions/setup-java@v4` | Android 构建所需 JDK 17 |
| `android-actions/setup-android@v3` | Android SDK/NDK |
| `docker/login-action@v3`、`docker/build-push-action@v6` | GHCR 镜像构建推送 |
| `softprops/action-gh-release@v2` | 创建 GitHub Release |

### 2.4 语法自检（提交前）

```bash
# 用 Node 的 js-yaml 快速验证（本项目 node_modules 已含）
node -e "
const y = require('js-yaml'), fs = require('fs');
for (const f of ['.github/workflows/release.yml', '.github/workflows/ci.yml']) {
  y.load(fs.readFileSync(f, 'utf8'));
  console.log(f + ' OK');
}"
```

> ⚠️ 曾踩坑：`run: |` 块内脚本行缩进不得低于块首行缩进，否则 YAML 解析失败（见 `README` 中「常见失败」表第 1 行）。

---

## 3. release.yml 详解

### 3.1 触发与全局配置

```yaml
on:
  workflow_dispatch:        # 手动触发：Actions 页 → Run workflow
  push:
    tags: ['v*']            # 推送 v 开头标签自动触发

env:
  EASYTIER_CORE_VERSION: v2.6.4   # easytier-core 版本单点管理

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: true        # 同 ref 并发时取消旧运行
```

- `env` 同时传给 Docker 构建（`build-args`），改版本只需改这一处。
- **版本号与标签**：`tauri.conf.json` 的 `version`（0.1.0）与标签 `v*` 不强制一致；建议发布时同步（如 `v0.1.0`）。

### 3.2 fetch-easytier（下载并校验内核归档）

```yaml
jobs.fetch-easytier:
  runs-on: ubuntu-latest
  steps:
    - checkout
    - Download easytier-core archives  # 循环下载 5 平台 zip 到 src-tauri/resources/bin/
    - Verify checksums                  # 通过 GitHub Release API 的 digest 校验 sha256
    - upload-artifact → easytier-bin
```

要点：

- 归档命名 `easytier-{os}-{arch}-v{VERSION}.zip` 与 `src-tauri/src/easytier/downloader.rs` 的解析前缀 `easytier-{platform}-v` **严格一致**，改名会静默失败（运行时找不到内置内核）。
- 校验脚本：取 `api.github.com/repos/EasyTier/EasyTier/releases/tags/{VERSION}` 的 `assets[].digest`（sha256），逐一比对，不匹配即 `exit 1`。
- ⚠️ 未来若新增 `windows-arm64` 打包：EasyTier 资产名为 `easytier-windows-arm64-*.zip`，而 `detect_platform()` 返回 `windows-aarch64`，需在下载循环里对该平台做 URL 映射（downloader 递归找 `easytier-core.exe`，内部目录名无关）。

### 3.3 build-desktop（桌面矩阵）

```yaml
strategy:
  fail-fast: false          # 某平台失败不取消其他平台
  matrix:
    include:
      - { os: macos-latest,          target: aarch64-apple-darwin,     bundle: dmg }
      - { os: macos-latest,          target: x86_64-apple-darwin,      bundle: dmg }
      - { os: windows-latest,        target: x86_64-pc-windows-msvc,   bundle: msi }
      - { os: ubuntu-latest,         target: x86_64-unknown-linux-gnu, bundle: deb }
      - { os: ubuntu-24.04-arm,      target: aarch64-unknown-linux-gnu,bundle: deb }
```

步骤链：checkout → Node22+pnpm9（带缓存）→ Rust stable + target（带 rust-cache）→ 下载 easytier-bin → （ubuntu 装 webkit 等系统依赖）→ `pnpm install --frozen-lockfile` → `pnpm tauri build`。

- **不需要显式 `pnpm build`**：`tauri.conf.json` 的 `beforeBuildCommand: "pnpm build"` 会在 `tauri build` 时自动执行（已移除重复步骤）。
- `ubuntu-24.04-arm`：GitHub 官方 ARM64 标准 runner（2026-01 起私有仓库可用），无需特殊配置。
- 无签名说明：macOS dmg / Windows msi 均未签名，首次安装会提示「无法验证开发者」，需右键打开或 `xattr -cr`。

### 3.4 build-appimage

同 build-desktop，产物追加 Linux AppImage 格式（`--bundles appimage`）。

### 3.5 build-android

```yaml
- pnpm tauri android init                   # 生成 gen/android（.gitignore 已排除，可反复生成）
- bash scripts/mobile-permissions.sh        # 注入相机权限
- pnpm tauri android build --apk            # 输出 3 ABI
- upload-gen/android/app/build/outputs/apk/**/*.apk
```

- `mobile-permissions.sh` 注入目标（已修正，与 Tauri 2 实际生成路径一致）：
  - Android：`src-tauri/gen/android/app/src/main/AndroidManifest.xml`
  - iOS：`src-tauri/gen/apple/homeTier_iOS/Info.plist`
- 当前 APK 未签名：可安装调试，正式分发需 keystore（见 §6 签名方案）。

### 3.6 build-ios

```yaml
- pnpm tauri ios init
- bash scripts/mobile-permissions.sh
- pnpm tauri ios build --no-sign      # 无签名构建，产物为 .app（非 ipa）
- upload src-tauri/gen/apple/build/**/*.app
```

- 无签名 .app 只能本地越狱/开发者模式使用；上架需 Apple Developer 账号签名（见 §6）。

### 3.7 build-docker（GHCR 镜像）

```yaml
permissions:
  contents: read
  packages: write            # 推送 ghcr.io 所需权限

- docker/login-action（registry: ghcr.io，GITHUB_TOKEN）
- docker/build-push-action：
    tags: ghcr.io/<owner>/<repo>:latest + :<ref_name>
    build-args: EASYTIER_CORE_VERSION=${{ env.EASYTIER_CORE_VERSION }}
```

- 注意：`workflow_dispatch` 手动触发时也会 push（`latest` + 分支名）。
- 首次推送后需在仓库 `Packages` 页确认镜像可见；`GITHUB_TOKEN` 自动有权限，无需额外 PAT。

### 3.8 release（汇总发布）

```yaml
needs: [build-desktop, build-appimage, build-android, build-ios]
if: startsWith(github.ref, 'refs/tags/')   # 仅标签推送时执行
- download-artifact → artifacts（merge-multiple）
- softprops/action-gh-release：自动生成 Release Notes + 附带全部安装包
```

手动触发时该 job 自动跳过（无 tag），构建产物保留在 Actions 页 90 天 / 下载。

---

## 4. ci.yml 详解（日常质量门禁）

```yaml
on: [push, pull_request]

jobs:
  frontend:                  # ubuntu-latest
    - pnpm install --frozen-lockfile
    - pnpm lint              # eslint src/（0 error 通过；warnings 不阻断）
    - pnpm build             # tsc --noEmit && vite build
  backend:                   # ubuntu-latest
    - apt: libssl-dev
    - cargo check --all-targets   # working-directory: src-tauri
```

- 两个 job 并行，支持缓存（pnpm node_modules / cargo 增量）。
- `cargo check --all-targets` 会编译测试目标——若测试代码引用了已改结构体，需同步补齐字段（本次已修复 `share.rs` 测试中 `ShareInfo` 缺 `name` 字段）。

---

## 5. 缓存机制

| 缓存 | 配置 | 命中收益 |
|---|---|---|
| pnpm | `setup-node@v4` + `cache: pnpm` + `cache-dependency-path: pnpm-lock.yaml` | 依赖安装秒级 |
| cargo | `swatinem/rust-cache@v2` + `workspaces: src-tauri -> target` | 增量编译，桌面构建 40min → 15–20min |

- rust-cache 的 `workspaces` 语法：`<cargo 目录> -> <target 相对路径>`，本项目 Cargo 工程在 `src-tauri/`，target 位于 `src-tauri/target`。
- 缓存 key 自动包含 target/工具链，矩阵各平台互不混淆。

---

## 6. 官方发布流程（写操作）

```bash
# 1. 本地确认版本号（package.json / src-tauri/Cargo.toml / tauri.conf.json 一致）
# 2. 打标签并推送（触发 release.yml 全流水线）
git tag v0.1.0
git push origin v0.1.0

# 3. Actions 页跟踪进度；完成后 release 页预览 Release Notes 与安装包
# 4. 手动触发（可选）：Actions → Release workflow → Run workflow
```

产物速查：

| 平台 | 产物 | 说明 |
|---|---|---|
| macOS Intel/Apple Silicon | `homeTier-x86_64.dmg` / `homeTier-aarch64.dmg` | 未签名 |
| Windows x64 | `homeTier-x86_64.msi` | 未签名 |
| Linux x64 / arm64 | `homeTier-x86_64.deb` / `homeTier-aarch64.deb` | |
| Linux 通用 | `homeTier.AppImage` | |
| Android | `homeTier-android.apk`（x86_64 / armv7 / arm64 3 ABI） | 未签名 |
| iOS | `homeTier-ios`（.app） | 无签名 |
| 服务器 | `ghcr.io/gmtfHomeHub/homeTier:latest` | 用于 `--server` 模式部署 |

---

## 7. 签名方案（按需启用，非本次执行范围）

### 7.1 Android（推荐优先，成本低）

1. 生成 keystore：`keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload`
2. 仓库 Secrets 添加：`ANDROID_KEYSTORE_BASE64`（`base64 upload-keystore.jks`）、`ANDROID_KEYSTORE_PASSWORD`、`ANDROID_KEY_ALIAS`
3. workflow 在 build-android 中：decode → 写 `src-tauri/gen/android/keystore.properties`：
   ```properties
   password=<secrets.ANDROID_KEYSTORE_PASSWORD>
   keyAlias=<secrets.ANDROID_KEY_ALIAS>
   storeFile=<keystore 路径>
   ```
   Tauri 生成的 `build.gradle.kts` 已支持读取该文件自动签名。

### 7.2 macOS 签名 + 公证

需 Apple Developer 账号（$99/y）。流程：p12 证书（base64 存 secret）→ 导入钥匙串 → `tauri.conf.json` 配 `bundle.macOS.signingIdentity` + `developmentTeam` → `xcrun notarytool` 公证并 stapler。整体工作量较大，暂缓。

### 7.3 Windows 签名

EV 代码签名证书昂贵；推荐 Azure Trusted Signing（`signtool sign /fdi` 免证书托管）。暂缓。

---

## 8. 常见失败排查表

| 现象 | 原因 | 处理 |
|---|---|---|
| workflow YAML 解析失败（`can not read a block mapping entry`） | `run: \|` 块内行缩进低于首行 | 统一所有脚本行缩进（≥块首行），用 §2.4 脚本自检 |
| fetch-easytier 下载 404 | EasyTier 该版本无对应平台资产 / 版本号笔误 | 核对 `EASYTIER_CORE_VERSION` 与 [EasyTier Releases](https://github.com/EasyTier/EasyTier/releases) |
| `CHECKSUM MISMATCH` | 资产 digest 与下载内容不一致 | 重新运行（镜像缓存异常）；仍失败则检查 API 限流（匿名 60 次/h，勿高频触发） |
| apt 安装失败（ubuntu） | 网络抖动 / 包源变更 | 重试 job；包名与 ubuntu 24.04 核对 |
| Android：找不到 SDK / NDK | setup-android 默认版本漂移 | 重试；必要时锁定 `ndk-version` 参数 |
| 浏览器下载 image 提示损坏 / 无法验证 | 未签名 | 右键→打开；`xattr -cr <app>`（mac）或 IE 增强设置（win） |
| GHCR push 403 | `packages: write` 权限缺失 | 确认 job 级 `permissions` 声明 |
| iOS/Android 相机权限不生效 | 注入路径与生成结构不一致 | 检查 `scripts/mobile-permissions.sh` 输出的 WARN；路径应以 `tauri android/ios init` 实生成为准 |
| artifacts 为空（warn） | 平台构建产物路径不符 | 用 `Locate artifacts` 步骤的 find 输出核对 bundle 目录 |

---

## 9. 已知边界与注意事项

1. **iOS 无签名**：产物仅用于 CI 冒烟，分发前必须补签名。
2. **docker 镜像的 dispatch 行为**：手动触发也会 push `latest`+分支名，适合验证镜像构建；如不希望，可加 `if: github.event_name != 'workflow_dispatch'` 到 push 步骤。
3. **Windows ARM64 桌面包**：当前矩阵不含；若加入需同时处理 easytier 资产名映射（§3.2）。
4. **供应链校验**：easytier 归档已做 sha256 校验；如进一步加固可加 `actions/attest-build-provenance` 与 cosign。
5. **移动端工程每次 init 重建**：`gen/` 不入库（.gitignore），`mobile-permissions.sh` 在 init 后运行，路径已按 Tauri 2 模板校准。

---

## 10. 参考

- GitHub Actions 文档：https://docs.github.com/actions
- Tauri 分发：https://tauri.app/distribute/（sign / release / updater）
- EasyTier Releases：https://github.com/EasyTier/EasyTier/releases
- swatinem/rust-cache：https://github.com/swatinem/rust-cache