#!/bin/bash
# 校验 Android 工程的关键注入是否生效（Kotlin 插件文件 / manifest 权限与 service / MainActivity）。
# 必须在 fix-android-mainactivity.sh、fix-android-build-gradle.sh、mobile-permissions.sh 全部执行之后、
# pnpm tauri android build 之前调用。任何一项缺失即 fail-fast，避免把缺件的 APK 打出来。
#
# 背景：mobile-permissions.sh / fix-android-mainactivity.sh 里有多处 `|| echo WARN` 静默容错，
# 若某步在 CI 里因路径漂移而没生效，APK 会缺 Kotlin 类 / manifest service / 权限，
# 在真机上表现为「VPN connection failed」。本脚本在构建前把这类缺口直接挡下来。

set -euo pipefail

ANDROID_KOTLIN_DIR="src-tauri/gen/android/app/src/main/java/com/hometier/app"
ANDROID_MANIFEST="src-tauri/gen/android/app/src/main/AndroidManifest.xml"
FAIL=0

echo "[verify-android-injection] Checking generated Android project at src-tauri/gen/android/"

# 1. Kotlin 插件 / VPN / 屏幕共享源码已复制进工程
if [ ! -d "$ANDROID_KOTLIN_DIR" ]; then
  echo "ERROR: Kotlin 源码目录不存在: $ANDROID_KOTLIN_DIR（tauri android init 未执行？）"
  FAIL=1
else
  for f in HomeTierVpnService.kt HomeTierVpnServicePlugin.kt MainActivity.kt; do
    if [ ! -f "$ANDROID_KOTLIN_DIR/$f" ]; then
      echo "ERROR: $f 缺失（$ANDROID_KOTLIN_DIR/$f）"
      FAIL=1
    else
      echo "OK: $f 存在"
    fi
  done
  if [ ! -f "$ANDROID_KOTLIN_DIR/screen/ScreenShareManager.kt" ]; then
    echo "ERROR: screen/ScreenShareManager.kt 缺失"
    FAIL=1
  else
    echo "OK: screen/ScreenShareManager.kt 存在"
  fi
fi

# 2. MainActivity 必须挂载 TauriEventBus（否则 Kotlin→Rust 的 tun-ready/state 事件无法送达）
if [ -f "$ANDROID_KOTLIN_DIR/MainActivity.kt" ]; then
  if ! grep -q 'TauriEventBus.attach' "$ANDROID_KOTLIN_DIR/MainActivity.kt"; then
    echo "ERROR: MainActivity.kt 缺少 TauriEventBus.attach（fix-android-mainactivity.sh 未生效？）"
    FAIL=1
  else
    echo "OK: MainActivity.kt 已挂载 TauriEventBus"
  fi
fi

# 3. AndroidManifest 必须包含 VpnService 声明与关键权限
if [ ! -f "$ANDROID_MANIFEST" ]; then
  echo "ERROR: AndroidManifest.xml 不存在"
  FAIL=1
else
  if ! grep -q 'com.hometier.app.HomeTierVpnService' "$ANDROID_MANIFEST"; then
    echo "ERROR: AndroidManifest 缺少 HomeTierVpnService service 声明（mobile-permissions.sh 未生效？）"
    FAIL=1
  else
    echo "OK: AndroidManifest 已声明 HomeTierVpnService"
  fi
  for p in INTERNET BIND_VPN_SERVICE FOREGROUND_SERVICE FOREGROUND_SERVICE_SYSTEM_EXEMPTED; do
    if ! grep -q "android.permission.$p" "$ANDROID_MANIFEST"; then
      echo "ERROR: AndroidManifest 缺少权限 android.permission.$p"
      FAIL=1
    else
      echo "OK: 权限 $p 存在"
    fi
  done
fi

# 4. build.gradle.kts 必须启用 cleartext traffic（否则 WebView 加载 127.0.0.1 代理报 ERROR_CLEARTEXT_NOT_PERMITTED）
BUILD_GRADLE="src-tauri/gen/android/app/build.gradle.kts"
if [ ! -f "$BUILD_GRADLE" ]; then
  echo "ERROR: build.gradle.kts 不存在"
  FAIL=1
elif ! grep -q 'manifestPlaceholders\["usesCleartextTraffic"\] = "true"' "$BUILD_GRADLE"; then
  echo "ERROR: build.gradle.kts 未启用 cleartext traffic（fix-android-build-gradle.sh 未生效？）"
  FAIL=1
else
  echo "OK: build.gradle.kts 已启用 cleartext traffic (usesCleartextTraffic=true)"
fi

# 5. build.gradle.kts 必须包含 ML Kit bundled model 修复（否则无 GMS 设备扫码永远无响应）
# fix-android-build-gradle.sh 会排除 play-services-mlkit-barcode-scanning（轻量模型，依赖 GMS）
# 并加入 com.google.mlkit:barcode-scanning（内置模型，全设备可用）。
# 若此步骤未生效，APK 在无 Google Play Services 的设备上 scanner.process() 静默失败。
if [ ! -f "$BUILD_GRADLE" ]; then
  echo "ERROR: build.gradle.kts 不存在（重复检查）"
  FAIL=1
elif ! grep -q 'com.google.mlkit:barcode-scanning' "$BUILD_GRADLE"; then
  echo "ERROR: build.gradle.kts 缺少 com.google.mlkit:barcode-scanning（ML Kit 内置模型修复未生效？）"
  FAIL=1
else
  echo "OK: build.gradle.kts 已引入 ML Kit 内置模型 (com.google.mlkit:barcode-scanning)"
fi
if [ -f "$BUILD_GRADLE" ] && ! grep -q 'play-services-mlkit-barcode-scanning' "$BUILD_GRADLE"; then
  echo "WARN: build.gradle.kts 中未见轻量模型排除项 exclude(play-services-mlkit-barcode-scanning)，请确认 fix 脚本已应用"
fi

if [ "$FAIL" -ne 0 ]; then
  echo ""
  echo "[verify-android-injection] ❌ 校验失败：Android 工程存在缺失，终止构建（APK 会缺 VPN 组件）"
  exit 1
fi

echo ""
echo "[verify-android-injection] ✅ Android 注入校验全部通过"
