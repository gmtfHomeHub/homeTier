#!/bin/bash
# 修正 `pnpm tauri android init` 生成的 Android 工程：
#   1. 对齐实际安装的 NDK 版本（CI runner 上是 29.x，而非 workflow 里声明的 25.x）
#   2. 注入 release 签名配置（hometier keystore）
#   3. 启用 cleartext traffic（WebView 访问 127.0.0.1 内部代理）
#   4. 使用 ML Kit 内置条码模型（不依赖 Google Play Services）
#
# ⚠️ 不修改 buildSrc/.../RustPlugin.kt：
#   模板里 ABI product flavor 列表用 defaultArchList（android init 时硬编码），
#   而 rustBuild 任务接线用 archList（构建期 `-ParchList=`，来自 CLI 的 --target），
#   两者用 targetPair.index 交叉索引。删掉 defaultArchList 里的 x86 会让 archList
#   的下标越过 flavor 列表 → tasks["mergeX86...JniLibFolders"] 找不到 → 配置期崩溃。
#   保留 x86 flavor 无害：它只能产出一个不带 native 库的空 APK，CI 侧按 ABI 过滤丢弃。
#
# ⚠️ per-ABI 拆分由 Tauri CLI 原生 flag 完成：
#     pnpm tauri android build --apk --target aarch64 armv7 x86_64 --split-per-abi
#   CLI 会向 Gradle 传 `-PabiList/-ParchList/-PtargetList`，而 Tauri 模板的
#   buildSrc/.../RustPlugin.kt 正是用 findProperty() 读取这三个属性来决定
#   生成哪些 rustBuild<Arch><Profile> 任务与 ABI product flavor。
#   因此**绝对不要**在 app/build.gradle.kts 里手写 ndk.abiFilters 或 splits.abi：
#   Tauri 模板已用 ABI product flavor 实现 per-ABI，两者同时存在会直接报
#   "Conflicting configuration: ... in ndk abiFilters cannot be present when splits abi filters are set"。

set -euo pipefail

BUILD_GRADLE="src-tauri/gen/android/app/build.gradle.kts"
PROGUARD_SRC="src-tauri/resources/gradle/proguard-rules.pro"

if [ ! -f "$BUILD_GRADLE" ]; then
    echo "ERROR: $BUILD_GRADLE not found（tauri android init 未执行？）"
    exit 1
fi

echo "[fix-android-build-gradle] Patching $BUILD_GRADLE ..."

# --- 1. NDK 版本对齐（NDK_HOME / ANDROID_NDK_HOME / 最新安装的 NDK） ---
NDK_PATH="${NDK_HOME:-}"
if [ -z "$NDK_PATH" ] || [ ! -d "$NDK_PATH" ]; then
    NDK_PATH="${ANDROID_NDK_HOME:-}"
fi
if [ -z "$NDK_PATH" ] || [ ! -d "$NDK_PATH" ]; then
    NDK_PATH="$(ls -d /usr/local/lib/android/sdk/ndk/*/ 2>/dev/null | sort -V | tail -1 | sed 's:/*$::')"
fi

if [ -n "$NDK_PATH" ] && [ -d "$NDK_PATH" ]; then
    ACTUAL_NDK=$(basename "$NDK_PATH")
    CURRENT_NDK=$(sed -n 's/.*ndkVersion = "\([^"]*\)".*/\1/p' "$BUILD_GRADLE" | head -1)
    if [ -n "$CURRENT_NDK" ] && [ "$CURRENT_NDK" != "$ACTUAL_NDK" ]; then
        sed -i "s/ndkVersion = \"$CURRENT_NDK\"/ndkVersion = \"$ACTUAL_NDK\"/" "$BUILD_GRADLE"
        echo "[fix-android-build-gradle] ndkVersion: $CURRENT_NDK -> $ACTUAL_NDK"
    else
        echo "[fix-android-build-gradle] ndkVersion 无需修改: ${CURRENT_NDK:-<empty>}"
    fi
else
    echo "[fix-android-build-gradle] WARN: 未找到 NDK，跳过 ndkVersion 对齐"
fi

# --- 2. keystore 复制到生成的工程内（两种约定路径都覆盖，见下方 keystore.properties） ---
if [ -f "src-tauri/keystore/release.keystore" ]; then
    mkdir -p src-tauri/gen/android/keystore src-tauri/gen/android/app/keystore
    cp src-tauri/keystore/release.keystore src-tauri/gen/android/keystore/release.keystore
    cp src-tauri/keystore/release.keystore src-tauri/gen/android/app/keystore/release.keystore
    echo "[fix-android-build-gradle] keystore 已复制到 gen/android/{,app/}keystore/"
else
    echo "[fix-android-build-gradle] WARN: src-tauri/keystore/release.keystore 不存在，跳过复制"
fi

# --- 3. proguard-rules.pro 复制到 app 模块（模板用 fileTree("**/*.pro") 收拢） ---
if [ -f "$PROGUARD_SRC" ]; then
    cp "$PROGUARD_SRC" src-tauri/gen/android/app/proguard-rules.pro
    echo "[fix-android-build-gradle] proguard-rules.pro 已复制到 app/"
else
    echo "[fix-android-build-gradle] WARN: $PROGUARD_SRC 不存在，跳过复制"
fi

# --- 4. 启用 cleartext traffic ---
if grep -q 'manifestPlaceholders\["usesCleartextTraffic"\] = "false"' "$BUILD_GRADLE"; then
    sed -i 's/manifestPlaceholders\["usesCleartextTraffic"\] = "false"/manifestPlaceholders["usesCleartextTraffic"] = "true"/' "$BUILD_GRADLE"
    echo "[fix-android-build-gradle] 已启用 usesCleartextTraffic=true"
else
    echo "[fix-android-build-gradle] usesCleartextTraffic 已是 true 或未找到 placeholder"
fi

# --- 5. Python 结构化修改：去掉 x86 flavor / ML Kit 内置模型 / 签名+R8 注入 / keystore.properties ---
python3 - <<'PYEOF'
import os
import re
import sys

BUILD_GRADLE = "src-tauri/gen/android/app/build.gradle.kts"
SIGNING_SRC = "src-tauri/resources/gradle/signing_config.gradle.kts"
MARKER = "// homeTier: injected signing + R8 config"


def log(msg):
    print(f"[fix-android-build-gradle] {msg}")


# ---------- 5.1 ML Kit 内置条码模型 ----------
with open(BUILD_GRADLE, encoding="utf-8") as f:
    content = f.read()

if "com.google.mlkit:barcode-scanning" in content:
    log("ML Kit 内置模型已存在，跳过")
else:
    mlkit_cfg = (
        '// --- ML Kit 内置模型（排除依赖 GMS 的 thin model） ---\n'
        'configurations.all {\n'
        '    exclude(group = "com.google.android.gms", module = "play-services-mlkit-barcode-scanning")\n'
        '}\n\n'
    )
    m = re.search(r"^dependencies\s*\{", content, flags=re.M)
    if not m:
        log("WARN: 未找到 dependencies 块，无法注入 ML Kit")
    else:
        content = content[: m.start()] + mlkit_cfg + content[m.start():]
        m2 = re.search(r"^dependencies\s*\{", content, flags=re.M)
        ins = m2.end()
        content = (
            content[:ins]
            + '\n    implementation("com.google.mlkit:barcode-scanning:17.2.0")'
            + content[ins:]
        )
        log("已注入 ML Kit 内置模型 (com.google.mlkit:barcode-scanning:17.2.0)")

# ---------- 5.2 注入签名 + R8 配置（apply(from=tauri.build.gradle.kts) 之后） ----------
if MARKER in content:
    log("签名/R8 配置已注入，跳过")
elif os.path.exists(SIGNING_SRC):
    with open(SIGNING_SRC, encoding="utf-8") as f:
        signing = f.read()
    target = 'apply(from = "tauri.build.gradle.kts")'
    if target in content:
        content = content.replace(target, target + "\n\n" + MARKER + "\n" + signing, 1)
        log("签名/R8 配置已注入（apply 之后）")
    else:
        content += "\n\n" + MARKER + "\n" + signing + "\n"
        log("WARN: 未找到 apply(from=...) 行，签名配置追加到文件末尾")
else:
    log(f"WARN: {SIGNING_SRC} 不存在，跳过签名注入")

with open(BUILD_GRADLE, "w", encoding="utf-8") as f:
    f.write(content)

# ---------- 5.3 keystore.properties（兼容模板自带的 signingConfigs 读取方式） ----------
ks = "src-tauri/gen/android/keystore/release.keystore"
if os.path.exists(ks):
    store_pw = os.environ.get("KEYSTORE_PASSWORD", "")
    key_pw = os.environ.get("KEY_PASSWORD", "")
    with open("src-tauri/gen/android/keystore.properties", "w", encoding="utf-8") as f:
        f.write(
            "storeFile=keystore/release.keystore\n"
            f"storePassword={store_pw}\n"
            "keyAlias=hometier\n"
            f"keyPassword={key_pw}\n"
        )
    log("已写入 gen/android/keystore.properties")

log("完成")
PYEOF

echo "[fix-android-build-gradle] 完成"
