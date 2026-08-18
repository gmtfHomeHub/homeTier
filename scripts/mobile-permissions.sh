#!/bin/bash
# 注入移动端相机权限到 Tauri 生成的原生工程中
# 在 release workflow 的 android/ios 作业中，pnpm tauri init 之后调用

set -euo pipefail

ANDROID_MANIFEST="src-tauri/gen/android/app/src/main/AndroidManifest.xml"
IOS_PLIST="src-tauri/gen/apple/homeTier_iOS/Info.plist"

# 防御：文件不存在时告警但不阻断（避免 build 因路径漂移被静默跳过注入）
[ -f "$ANDROID_MANIFEST" ] || echo "[mobile-permissions] WARN: $ANDROID_MANIFEST 不存在（tauri android init 未执行或结构变更）"
[ -f "$IOS_PLIST" ] || echo "[mobile-permissions] WARN: $IOS_PLIST 不存在（tauri ios init 未执行或结构变更）"

if [ -f "$ANDROID_MANIFEST" ]; then
    if ! grep -q 'android.permission.CAMERA' "$ANDROID_MANIFEST"; then
        sed -i '/<\/manifest>/i \
    <uses-permission android:name="android.permission.CAMERA" />' "$ANDROID_MANIFEST"
        echo "[mobile-permissions] Injected CAMERA permission into AndroidManifest.xml"
    else
        echo "[mobile-permissions] CAMERA permission already exists in AndroidManifest.xml"
    fi
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
fi