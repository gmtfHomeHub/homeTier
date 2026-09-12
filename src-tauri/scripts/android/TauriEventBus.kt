// TauriEventBus - 使用 evaluateJavascript 向 WebView 注入事件
// 避免依赖 triggerCallback 静态变量，更符合 Tauri 2 移动端事件桥标准
package com.hometier.app

import android.util.Log
import android.webkit.WebView

object TauriEventBus {
    private var webView: WebView? = null

    fun attach(wv: WebView) {
        webView = wv
        Log.d("TauriEventBus", "WebView attached")
    }

    fun detach() {
        webView = null
        Log.d("TauriEventBus", "WebView detached")
    }

    /**
     * 通过 Tauri 2 事件插件发出事件。
     *
     * 使用 plugin:event|emit 命令（非 plugin:event|listen），将事件投递到
     * Rust 侧 app_handle.listen() 注册的监听器。
     * payload 为合法 JSON 字符串，直接嵌入 JS 作为对象字面量。
     */
    fun emit(event: String, payload: String) {
        val wv = webView ?: run {
            Log.w("TauriEventBus", "WebView not attached, cannot emit $event")
            return
        }
        wv.post {
            val js = """
                (function(){
                    if (window.__TAURI_INTERNALS__) {
                        window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
                            event: '$event',
                            payload: $payload
                        });
                    } else {
                        console.warn('Tauri internals not available');
                    }
                })();
            """.trimIndent()
            wv.evaluateJavascript(js, null)
        }
        Log.d("TauriEventBus", "Emitted event: $event, payload: $payload")
    }
}
