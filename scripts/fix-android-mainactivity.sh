#!/bin/bash
# 修复 Tauri 2.x Android 模板生成的 MainActivity.kt
# 在 pnpm tauri android init 之后、mobile-permissions.sh 之前运行

set -euo pipefail

MAIN_ACTIVITY="src-tauri/gen/android/app/src/main/java/com/hometier/app/MainActivity.kt"

if [ ! -f "$MAIN_ACTIVITY" ]; then
    echo "[fix-android-mainactivity] WARN: $MAIN_ACTIVITY 不存在（tauri android init 未执行）"
    exit 0
fi

echo "[fix-android-mainactivity] Patching MainActivity.kt for Tauri 2.x..."

# 备份原文件
cp "$MAIN_ACTIVITY" "$MAIN_ACTIVITY.bak"

# 生成正确的 MainActivity.kt - 使用 onWebViewCreate 回调而非 onCreate
# 同时注册 HomeTierVpnServicePlugin
cat > "$MAIN_ACTIVITY" <<'EOF'
package com.hometier.app

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
    }

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        TauriEventBus.attach(webView)
    }

    override fun onDestroy() {
        TauriEventBus.detach()
        super.onDestroy()
    }
}
EOF

echo "[fix-android-mainactivity] MainActivity.kt patched successfully"