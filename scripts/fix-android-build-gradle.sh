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

# 将 keystore 复制到生成的 android 工程内（file("../keystore/release.keystore") 相对 app 模块）
if [ -f "src-tauri/keystore/release.keystore" ]; then
    mkdir -p src-tauri/gen/android/keystore
    cp src-tauri/keystore/release.keystore src-tauri/gen/android/keystore/release.keystore
    echo "[fix-android-build-gradle] Copied keystore to gen/android/keystore/"
else
    echo "[fix-android-build-gradle] WARN: src-tauri/keystore/release.keystore 不存在，跳过复制"
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
