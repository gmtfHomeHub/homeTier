#!/bin/bash
# Fix Android build.gradle.kts for per-ABI APK splits (robust version)
# Key: clear ndk.abiFilters AFTER tauri.apply using abiFilters.clear()

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

cp "$BUILD_GRADLE" "$BUILD_GRADLE.bak"

# --- NDK version fix (supports NDK_HOME, ANDROID_NDK_HOME, fallback) ---
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
        echo "[fix-android-build-gradle] Updated ndkVersion: $CURRENT_NDK -> $ACTUAL_NDK"
    else
        echo "[fix-android-build-gradle] ndkVersion already correct: ${CURRENT_NDK:-<empty>}"
    fi
else
    echo "[fix-android-build-gradle] NDK path not found, skipping NDK version fix"
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
    echo "[fix-android-build-gradle] Enabled cleartext traffic (usesCleartextTraffic=true)"
else
    echo "[fix-android-build-gradle] usesCleartextTraffic placeholder not found or already true"
fi

# --- Python structural edits: ML Kit, ABI splits ---
python3 << 'PYEOF'
import re
import os

path = 'src-tauri/gen/android/app/build.gradle.kts'
with open(path, 'r') as f:
    content = f.read()

# ===== 1. ML Kit: 切换到内置模型 =====
if 'com.google.mlkit:barcode-scanning' not in content:
    mlkit_config = '''
    // --- ML Kit bundled model (no Google Play Services dependency) ---
    configurations.all {
        exclude(group = "com.google.android.gms", module = "play-services-mlkit-barcode-scanning")
    }
    dependencies {
        implementation("com.google.mlkit:barcode-scanning:17.2.0")
    }
'''
    dep_match = re.search(r'(dependencies\s*\{[^}]*\})', content, re.DOTALL)
    if dep_match:
        old_dep = dep_match.group(1)
        new_dep = old_dep.replace('dependencies {', 'dependencies {\n' + mlkit_config.strip())
        content = content.replace(old_dep, new_dep)
        print('[fix-android-build-gradle] Added ML Kit bundled model to dependencies')
    else:
        content += '\n' + mlkit_config + '\n'
        print('[fix-android-build-gradle] Appended ML Kit bundled model config')

# ===== 2. ABI splits: 插入到现有 android { } 块内部 =====
if 'splits {' not in content:
    abi_splits_config = '''
    // --- ABI splits: 生成 per-ABI APK，避免 universal APK 过大 ---
    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "armeabi-v7a", "x86_64")
            isUniversalApk = true  // 保留 universal variant 任务，兼容 Tauri 引用
        }
    }
'''
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

# ===== 3. 创建消费者 ProGuard 规则目录和空文件 =====
consumer_dir = 'src-tauri/gen/android/app/consumer-proguard-rules'
os.makedirs(consumer_dir, exist_ok=True)
for plugin in [
    'tauri-plugin-clipboard-manager',
    'tauri-plugin-dialog',
    'tauri-plugin-notification',
    'tauri-plugin-shell',
]:
    rules_file = os.path.join(consumer_dir, f'{plugin}.pro')
    if not os.path.exists(rules_file):
        with open(rules_file, 'w') as f:
            f.write(f'# Empty consumer ProGuard rules for {plugin}\n')
        print(f'[fix-android-build-gradle] Created empty consumer rules: {rules_file}')

with open(path, 'w') as f:
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

# Insert signing config + abiFilters.clear() AFTER tauri.apply line
cat > /tmp/insert_signing_and_clear.py << 'PYEOF'
import sys

with open('src-tauri/gen/android/app/build.gradle.kts', 'r') as f:
    content = f.read()

with open('src-tauri/resources/gradle/signing_config.gradle.kts', 'r') as f:
    signing_config = f.read()

if 'signingConfigs' in content:
    print('Signing config already present')
    sys.exit(0)

# Insert AFTER apply(from = "tauri.build.gradle.kts")
lines = content.split('\n')
new_lines = []
inserted = False

for line in lines:
    new_lines.append(line)
    if 'apply(from = "tauri.build.gradle.kts")' in line and not inserted:
        new_lines.append('')
        new_lines.append(signing_config)
        new_lines.append('')
        # CRITICAL: Clear abiFilters AFTER tauri.apply using abiFilters.clear()
        # abiFilters is a val MutableSet<String>, cannot reassign, must call clear()
        new_lines.append('android {')
        new_lines.append('    defaultConfig {')
        new_lines.append('        ndk {')
        new_lines.append('            abiFilters.clear()')
        new_lines.append('        }')
        new_lines.append('    }')
        new_lines.append('}')
        inserted = True

if not inserted:
    print('WARNING: Could not find apply line, appending at end')
    new_lines.append('')
    new_lines.append(signing_config)
    new_lines.append('')
    new_lines.append('android {')
    new_lines.append('    defaultConfig {')
    new_lines.append('        ndk {')
    new_lines.append('            abiFilters.clear()')
    new_lines.append('        }')
    new_lines.append('    }')
    new_lines.append('}')

with open('src-tauri/gen/android/app/build.gradle.kts', 'w') as f:
    f.write('\n'.join(new_lines))

print('Successfully patched build.gradle.kts with signing config and abiFilters.clear()')
PYEOF

python3 /tmp/insert_signing_and_clear.py