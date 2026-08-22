#!/bin/bash
# 修复 Tauri 2.x Android 模板生成的 HomeTierVpnServicePlugin.kt
# 在 pnpm tauri android init 之后、Fix MainActivity 之前运行

set -euo pipefail

PLUGIN_FILE="src-tauri/gen/android/app/src/main/java/com/hometier/app/HomeTierVpnServicePlugin.kt"

if [ ! -f "$PLUGIN_FILE" ]; then
    echo "[fix-android-plugin] WARN: $PLUGIN_FILE 不存在（tauri android init 未执行）"
    exit 0
fi

echo "[fix-android-plugin] Patching HomeTierVpnServicePlugin.kt import..."

# 备份原文件
cp "$PLUGIN_FILE" "$PLUGIN_FILE.bak"

# 修复导入路径：将 import com.hometier.app.ScreenShareManager 替换为 import com.hometier.app.screen.ScreenShareManager
sed -i 's/import com\.hometier\.app\.ScreenShareManager/import com.hometier.app.screen.ScreenShareManager/g' "$PLUGIN_FILE"

echo "[fix-android-plugin] HomeTierVpnServicePlugin.kt patched successfully"
