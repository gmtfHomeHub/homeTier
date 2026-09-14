#!/bin/bash
# Fix Android build.gradle.kts by properly integrating signing config

set -euo pipefail

BUILD_GRADLE="src-tauri/gen/android/app/build.gradle.kts"
SIGNING_CONFIG_FILE="src-tauri/resources/gradle/signing_config.gradle.kts"

if [ ! -f "$BUILD_GRADLE" ]; then
    echo "ERROR: $BUILD_GRADLE not found"
    exit 1
fi

if [ ! -f "$SIGNING_CONFIG_FILE" ]; then
    echo "ERROR: $SIGNING_CONFIG_FILE not found"
    exit 1
fi

echo "[fix-android-build-gradle] Patching $BUILD_GRADLE..."

# Backup
cp "$BUILD_GRADLE" "$BUILD_GRADLE.bak"

# --- NDK version fix: Tauri 默认写入的 ndkVersion 可能与 CI 实际安装的 NDK 不一致，
# 导致 Gradle 找不到指定 NDK 版本而失败。根据 NDK_HOME 自动校正。 ---
if [ -n "${NDK_HOME:-}" ] && [ -d "$NDK_HOME" ]; then
    ACTUAL_NDK=$(basename "$NDK_HOME")
    CURRENT_NDK=$(sed -n 's/.*ndkVersion = "\([^"]*\)".*/\1/p' "$BUILD_GRADLE" | head -1)
    if [ "$CURRENT_NDK" != "$ACTUAL_NDK" ]; then
        sed -i "s/ndkVersion = \"$CURRENT_NDK\"/ndkVersion = \"$ACTUAL_NDK\"/" "$BUILD_GRADLE"
        echo "[fix-android-build-gradle] Updated ndkVersion: $CURRENT_NDK -> $ACTUAL_NDK"
    else
        echo "[fix-android-build-gradle] ndkVersion already correct: $ACTUAL_NDK"
    fi
else
    echo "[fix-android-build-gradle] NDK_HOME not set, skipping NDK version fix"
fi

# 将 keystore 复制到生成的 android 工程内（file("../keystore/release.keystore") 相对 app 模块）
if [ -f "src-tauri/keystore/release.keystore" ]; then
    mkdir -p src-tauri/gen/android/keystore
    cp src-tauri/keystore/release.keystore src-tauri/gen/android/keystore/release.keystore
    echo "[fix-android-build-gradle] Copied keystore to gen/android/keystore/"
else
    echo "[fix-android-build-gradle] WARN: src-tauri/keystore/release.keystore 不存在，跳过复制"
fi

# 复制 proguard-rules.pro 到 app 模块目录（signing_config.gradle.kts 引用相对路径）
if [ -f "src-tauri/resources/gradle/proguard-rules.pro" ]; then
    cp src-tauri/resources/gradle/proguard-rules.pro src-tauri/gen/android/app/proguard-rules.pro
    echo "[fix-android-build-gradle] Copied proguard-rules.pro to gen/android/app/"
else
    echo "[fix-android-build-gradle] WARN: proguard-rules.pro 不存在，跳过复制"
fi

# 启用 cleartext traffic：HTTP 代理走 127.0.0.1 明文，Tauri 默认 release 继承 defaultConfig 的 "false"，
# 会导致 WebView 加载 http://127.0.0.1:port/__proxy__ 报 net::ERROR_CLEARTEXT_NOT_PERMITTED。
# 改 defaultConfig placeholder 为 true（debug 本就 true，release 继承 defaultConfig 即生效）。
if grep -q 'manifestPlaceholders\["usesCleartextTraffic"\] = "false"' "$BUILD_GRADLE"; then
    sed -i 's/manifestPlaceholders\["usesCleartextTraffic"\] = "false"/manifestPlaceholders["usesCleartextTraffic"] = "true"/' "$BUILD_GRADLE"
    echo "[fix-android-build-gradle] Enabled cleartext traffic for localhost proxy (usesCleartextTraffic=true)"
else
    echo "[fix-android-build-gradle] usesCleartextTraffic placeholder not found or already true"
fi

# --- ML Kit: 切换到内置模型（不依赖 Google Play Services） ---
# tauri-plugin-barcode-scanner 默认依赖 play-services-mlkit-barcode-scanning（轻量模型），
# 需要 Google Play Services。在无 GMS 的设备上（华为/国产 ROM），scan() 能打开相机但永远
# 无法识别二维码——ML Kit 条码模型从 GMS 加载失败，scanner.process() 静默失败。
# 解决：排除轻量模型，改用 com.google.mlkit:barcode-scanning（内置模型，~3MB，全设备可用）。
if ! grep -q 'com.google.mlkit:barcode-scanning' "$BUILD_GRADLE"; then
    cat >> "$BUILD_GRADLE" << 'MLKIT_EOF'

// --- ML Kit bundled model (no Google Play Services dependency) ---
// Replaces play-services-mlkit-barcode-scanning (thin model, requires GMS)
// with com.google.mlkit:barcode-scanning (bundled model, works on all devices)
configurations.all {
    exclude(group = "com.google.android.gms", module = "play-services-mlkit-barcode-scanning")
}
dependencies {
    implementation("com.google.mlkit:barcode-scanning:17.2.0")
}
MLKIT_EOF
    echo "[fix-android-build-gradle] Switched ML Kit to bundled model (no GMS dependency)"
else
    echo "[fix-android-build-gradle] ML Kit bundled model already present"
fi

# --- ABI splits: 生成 per-ABI APK，替代 universal APK ---
if ! grep -q 'splits {' "$BUILD_GRADLE"; then
    cat >> "$BUILD_GRADLE" << 'ABI_SPLITS_EOF'

// --- ABI splits: 生成 per-ABI APK，避免 universal APK 过大 ---
android {
    splits {
        abi {
            enable = true
            reset()
            include("arm64-v8a", "armeabi-v7a", "x86_64")
            universalApk = false
        }
    }
}
ABI_SPLITS_EOF
    echo "[fix-android-build-gradle] Added ABI splits for per-ABI APKs"
else
    echo "[fix-android-build-gradle] ABI splits already present"
fi

# --- Consumer ProGuard rules for Tauri plugins missing consumer-rules.pro ---
# 这些插件缺少 consumer-rules.pro，R8 会报警告；提供空规则文件避免警告
CONSUMER_RULES_DIR="src-tauri/gen/android/app/consumer-proguard-rules"
mkdir -p "$CONSUMER_RULES_DIR"

# 为缺少 consumer-rules.pro 的插件创建空规则文件
for plugin in "tauri-plugin-clipboard-manager" "tauri-plugin-dialog" "tauri-plugin-notification" "tauri-plugin-shell"; do
    RULES_FILE="$CONSUMER_RULES_DIR/${plugin}.pro"
    if [ ! -f "$RULES_FILE" ]; then
        echo "# Empty consumer ProGuard rules for $plugin (no special rules needed)" > "$RULES_FILE"
        echo "[fix-android-build-gradle] Created empty consumer rules: $RULES_FILE"
    fi
done

# 在 build.gradle.kts 中添加 consumerProguardFiles 指向这些规则
if ! grep -q 'consumerProguardFiles' "$BUILD_GRADLE"; then
    cat >> "$BUILD_GRADLE" << 'CONSUMER_PROGUARD_EOF'

// --- Consumer ProGuard rules for plugins missing consumer-rules.pro ---
tasks.withType(com.android.build.gradle.tasks.R8Task).configureEach {
    consumerProguardFiles(
        file("../consumer-proguard-rules/tauri-plugin-clipboard-manager.pro"),
        file("../consumer-proguard-rules/tauri-plugin-dialog.pro"),
        file("../consumer-proguard-rules/tauri-plugin-notification.pro"),
        file("../consumer-proguard-rules/tauri-plugin-shell.pro")
    )
}
CONSUMER_PROGUARD_EOF
    echo "[fix-android-build-gradle] Added consumerProguardFiles for missing plugin rules"
else
    echo "[fix-android-build-gradle] consumerProguardFiles already present"
fi

# Check if signing config already exists
if grep -q "signingConfigs" "$BUILD_GRADLE"; then
    echo "[fix-android-build-gradle] Signing config already present, skipping"
    exit 0
fi

# Read signing config content
SIGNING_CONFIG=$(cat "$SIGNING_CONFIG_FILE")

# Use Python to properly insert the signing config before the tauri apply line
python3 << 'EOF'
import sys

with open('src-tauri/gen/android/app/build.gradle.kts', 'r') as f:
    content = f.read()

# Read signing config
with open('src-tauri/resources/gradle/signing_config.gradle.kts', 'r') as f:
    signing_config = f.read()

# Check if already present
if 'signingConfigs' in content:
    print("Signing config already present")
    sys.exit(0)

# Insert signing config before the apply(from = "tauri.build.gradle.kts") line
# Find the line with apply(from = "tauri.build.gradle.kts")
lines = content.split('\n')
new_lines = []
inserted = False

for line in lines:
    if 'apply(from = "tauri.build.gradle.kts")' in line and not inserted:
        # Insert signing config before this line
        new_lines.append("")
        new_lines.append(signing_config)
        new_lines.append("")
        inserted = True
    new_lines.append(line)

if not inserted:
    print("WARNING: Could not find apply line, appending at end")
    new_lines.append("")
    new_lines.append(signing_config)

with open('src-tauri/gen/android/app/build.gradle.kts', 'w') as f:
    f.write('\n'.join(new_lines))

print("Successfully patched build.gradle.kts")
EOF
