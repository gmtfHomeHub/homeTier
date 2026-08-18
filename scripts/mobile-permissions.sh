#!/bin/bash
# 注入移动端权限到 Tauri 生成的原生工程中
# 在 release workflow 的 android/ios 作业中，pnpm tauri init 之后调用

set -euo pipefail

ANDROID_MANIFEST="src-tauri/gen/android/app/src/main/AndroidManifest.xml"
ANDROID_KOTLIN_DIR="src-tauri/gen/android/app/src/main/java/com/hometier/app"
IOS_PLIST="src-tauri/gen/apple/homeTier_iOS/Info.plist"

# 防御：文件不存在时告警但不阻断（避免 build 因路径漂移被静默跳过注入）
[ -f "$ANDROID_MANIFEST" ] || echo "[mobile-permissions] WARN: $ANDROID_MANIFEST 不存在（tauri android init 未执行或结构变更）"
[ -f "$IOS_PLIST" ] || echo "[mobile-permissions] WARN: $IOS_PLIST 不存在（tauri ios init 未执行或结构变更）"

if [ -f "$ANDROID_MANIFEST" ]; then
    # 1. CAMERA 权限（用于扫码）
    if ! grep -q 'android.permission.CAMERA' "$ANDROID_MANIFEST"; then
        sed -i '/<\/manifest>/i \
    <uses-permission android:name="android.permission.CAMERA" />' "$ANDROID_MANIFEST"
        echo "[mobile-permissions] Injected CAMERA permission into AndroidManifest.xml"
    else
        echo "[mobile-permissions] CAMERA permission already exists in AndroidManifest.xml"
    fi

    # 2. VPN 相关权限
    if ! grep -q 'android.permission.INTERNET' "$ANDROID_MANIFEST"; then
        sed -i '/<\/manifest>/i \
    <uses-permission android:name="android.permission.INTERNET" />' "$ANDROID_MANIFEST"
        echo "[mobile-permissions] Injected INTERNET permission into AndroidManifest.xml"
    fi

    # 3. BIND_VPN_SERVICE 权限（用于 VpnService）
    if ! grep -q 'android.permission.BIND_VPN_SERVICE' "$ANDROID_MANIFEST"; then
        sed -i '/<\/manifest>/i \
    <uses-permission android:name="android.permission.BIND_VPN_SERVICE" />' "$ANDROID_MANIFEST"
        echo "[mobile-permissions] Injected BIND_VPN_SERVICE permission into AndroidManifest.xml"
    else
        echo "[mobile-permissions] BIND_VPN_SERVICE permission already exists in AndroidManifest.xml"
    fi

    # 4. FOREGROUND_SERVICE 权限（Android 14+ 需要）
    if ! grep -q 'android.permission.FOREGROUND_SERVICE' "$ANDROID_MANIFEST"; then
        sed -i '/<\/manifest>/i \
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />' "$ANDROID_MANIFEST"
        echo "[mobile-permissions] Injected FOREGROUND_SERVICE permission into AndroidManifest.xml"
    fi

    # 5. 在 <application> 中注册 VpnService
    if ! grep -q 'com.hometier.app.HomeTierVpnService' "$ANDROID_MANIFEST"; then
        # 使用 sed 在 </application> 之前插入 service 声明
        sed -i '/<\/application>/i \
        <service\n            android:name="com.hometier.app.HomeTierVpnService"\n            android:permission="android.permission.BIND_VPN_SERVICE"\n            android:exported="true">\n            <intent-filter>\n                <action android:name="android.net.VpnService" />\n            </intent-filter>\n        </service>' "$ANDROID_MANIFEST"
        echo "[mobile-permissions] Injected HomeTierVpnService into AndroidManifest.xml"
    else
        echo "[mobile-permissions] HomeTierVpnService already registered in AndroidManifest.xml"
    fi
fi

# 复制 VpnService.kt 到生成的工程中
VPN_SERVICE_SOURCE="src-tauri/scripts/android/VpnService.kt"
VPN_SERVICE_DEST="$ANDROID_KOTLIN_DIR/VpnService.kt"

if [ -f "$VPN_SERVICE_SOURCE" ] && [ -d "$ANDROID_KOTLIN_DIR" ]; then
    if [ ! -f "$VPN_SERVICE_DEST" ] || ! cmp -s "$VPN_SERVICE_SOURCE" "$VPN_SERVICE_DEST"; then
        cp "$VPN_SERVICE_SOURCE" "$VPN_SERVICE_DEST"
        echo "[mobile-permissions] Copied VpnService.kt to $VPN_SERVICE_DEST"
    else
        echo "[mobile-permissions] VpnService.kt already up to date"
    fi
elif [ -f "$VPN_SERVICE_SOURCE" ]; then
    echo "[mobile-permissions] WARN: Kotlin directory $ANDROID_KOTLIN_DIR 不存在（tauri android init 可能未执行）"
fi

if [ -f "$IOS_PLIST" ]; then
    if ! grep -q 'NSCameraUsageDescription' "$IOS_PLIST"; then
        NODE_=$(which node || true)
        if [ -n "$NODE_" ]; then
            node -e "
const f=require('fs');
let c=f.readFileSync('$IOS_PLIST','utf8');
if(c.indexOf('NSCameraUsageDescription')===-1){
  c=c.replace('</dict></plist>',
    '<key>NSCameraUsageDescription</key><string>Camera access is needed to scan QR codes</string></dict></plist>');
  f.writeFileSync('$IOS_PLIST',c);
}
"
        else
            sed -i '/<\/dict>$/i \
    <key>NSCameraUsageDescription</key>\&#10;    <string>Camera access is needed to scan QR codes</string>' "$IOS_PLIST"
        fi
        echo "[mobile-permissions] Injected NSCameraUsageDescription into Info.plist"
    else
        echo "[mobile-permissions] Camera usage description already exists in Info.plist"
    fi

    # iOS Network Extension 权限（用于 NEPacketTunnelProvider）
    if ! grep -q 'com.apple.developer.networking.networkextension' "$IOS_PLIST"; then
        NODE_=$(which node || true)
        if [ -n "$NODE_" ]; then
            node -e "
const f=require('fs');
let c=f.readFileSync('$IOS_PLIST','utf8');
if(c.indexOf('com.apple.developer.networking.networkextension')===-1){
  c=c.replace('</dict></plist>',
    '<key>com.apple.developer.networking.networkextension</key><array><string>packet-tunnel-provider</string></array></dict></plist>');
  f.writeFileSync('$IOS_PLIST',c);
}
"
        fi
        echo "[mobile-permissions] Injected NetworkExtension entitlement into Info.plist"
    else
        echo "[mobile-permissions] NetworkExtension entitlement already exists in Info.plist"
    fi
fi