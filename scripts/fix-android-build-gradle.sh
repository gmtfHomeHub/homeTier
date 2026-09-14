#!/bin/bash
# Fix Android build.gradle.kts by properly integrating signing config and ABI splits

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

# --- NDK version fix ---
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

# 将 keystore 复制到生成的 android 工程内
if [ -f "src-tauri/keystore/release.keystore" ]; then
    mkdir -p src-tauri/gen/android/keystore
    cp src-tauri/keystore/release.keystore src-tauri/gen/android/keystore/release.keystore
    echo "[fix-android-build-gradle] Copied keystore to gen/android/keystore/"
else
    echo "[fix-android-build-gradle] WARN: src-tauri/keystore/release.keystore 不存在，跳过复制"
fi

# 复制 proguard-rules.pro 到 app 模块目录
if [ -f "src-tauri/resources/gradle/proguard-rules.pro" ]; then
    cp src-tauri/resources/gradle/proguard-rules.pro src-tauri/gen/android/app/proguard-rules.pro
    echo "[fix-android-build-gradle] Copied proguard-rules.pro to gen/android/app/"
else
    echo "[fix-android-build-gradle] WARN: proguard-rules.pro 不存在，跳过复制"
fi

# 启用 cleartext traffic
if grep -q 'manifestPlaceholders\["usesCleartextTraffic"\] = "false"' "$BUILD_GRADLE"; then
    sed -i 's/manifestPlaceholders\["usesCleartextTraffic"\] = "false"/manifestPlaceholders["usesCleartextTraffic"] = "true"/' "$BUILD_GRADLE"
    echo "[fix-android-build-gradle] Enabled cleartext traffic for localhost proxy (usesCleartextTraffic=true)"
else
    echo "[fix-android-build-gradle] usesCleartextTraffic placeholder not found or already true"
fi

# --- 使用 Python 进行所有结构化修改 ---
python3 << 'PYEOF'
import re
import sys

with open('src-tauri/gen/android/app/build.gradle.kts', 'r') as f:
    content = f.read()

# ===== 1. ML Kit: 切换到内置模型 =====
if 'com.google.mlkit:barcode-scanning' not in content:
    # 找到 dependencies { } 块，在里面添加
    mlkit_config = '''
    // --- ML Kit bundled model (no Google Play Services dependency) ---
    // Replaces play-services-mlkit-barcode-scanning (thin model, requires GMS)
    // with com.google.mlkit:barcode-scanning (bundled model, works on all devices)
    configurations.all {
        exclude(group = "com.google.android.gms", module = "play-services-mlkit-barcode-scanning")
    }
    dependencies {
        implementation("com.google.mlkit:barcode-scanning:17.2.0")
    }
'''
    # 尝试在现有 dependencies { } 块内插入，或在 android { } 后添加
    dep_match = re.search(r'(dependencies\s*\{[^}]*\})', content, re.DOTALL)
    if dep_match:
        # 在现有 dependencies 块内插入
        old_dep = dep_match.group(1)
        new_dep = old_dep.replace('dependencies {', 'dependencies {\n' + mlkit_config.strip())
        content = content.replace(old_dep, new_dep)
        print("[fix-android-build-gradle] Added ML Kit bundled model to dependencies")
    else:
        # 没有 dependencies 块，在 android { } 后添加
        android_end = content.rfind('}')
        if android_end >= 0:
            content = content[:android_end] + '\n' + mlkit_config + '\n' + content[android_end:]
            print("[fix-android-build-gradle] Added ML Kit bundled model after android block")
        else:
            print("[fix-android-build-gradle] WARNING: Could not find place to insert ML Kit config")

# ===== 2. ABI splits: 插入到现有 android { } 块内部 =====
if 'splits {' not in content:
    # 找到 android { ... } 块，在最后一个 } 前插入 splits 配置
    abi_splits_config = '''
    // --- ABI splits: 生成 per-ABI APK，避免 universal APK 过大 ---
    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "armeabi-v7a", "x86_64")
            isUniversalApk = false
        }
    }
'''
    # 找到 android { 的最后一个匹配的 }
    android_start = content.find('android {')
    if android_start >= 0:
        brace_count = 0
        insert_pos = -1
        for i, ch in enumerate(content[android_start:], start=android_start):
            if ch == '{':
                brace_count += 1
            elif ch == '}':
                brace_count -= 1
                if brace_count == 0:
                    insert_pos = i
                    break
        if insert_pos >= 0:
            content = content[:insert_pos] + '\n' + abi_splits_config + '\n' + content[insert_pos:]
            print("[fix-android-build-gradle] Added ABI splits inside android block")
        else:
            print("[fix-android-build-gradle] WARNING: Could not find android block end")
    else:
        print("[fix-android-build-gradle] WARNING: Could not find android block start")

# ===== 3. 创建消费者 ProGuard 规则目录和空文件（仅为了消除警告）=====
import os
CONSUMER_RULES_DIR = "src-tauri/gen/android/app/consumer-proguard-rules"
os.makedirs(CONSUMER_RULES_DIR, exist_ok=True)

for plugin in ["tauri-plugin-clipboard-manager", "tauri-plugin-dialog", "tauri-plugin-notification", "tauri-plugin-shell"]:
    rules_file = os.path.join(CONSUMER_RULES_DIR, f"{plugin}.pro")
    if not os.path.exists(rules_file):
        with open(rules_file, 'w') as f:
            f.write(f"# Empty consumer ProGuard rules for {plugin} (no special rules needed)\n")
        print(f"[fix-android-build-gradle] Created empty consumer rules: {rules_file}")

# ===== 4. 写回文件 =====
with open('src-tauri/gen/android/app/build.gradle.kts', 'w') as f:
    f.write(content)

print("[fix-android-build-gradle] Python modifications completed")
PYEOF

# --- Check if signing config already exists ---
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
lines = content.split('\n')
new_lines = []
inserted = False

for line in lines:
    if 'apply(from = "tauri.build.gradle.kts")' in line and not inserted:
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