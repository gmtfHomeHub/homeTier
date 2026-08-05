import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppLayout } from "./components/Layout/AppLayout";
import { TrayBridge } from "./components/Layout/TrayBridge";
import { SpaceList } from "./components/Space/SpaceList";
import { SpaceDetail } from "./components/Space/SpaceDetail";
import { AppBrowserView } from "./components/AppBrowser/AppBrowserView";
import { ChatView } from "./components/Chat/ChatView";
import { VoicePanel } from "./components/Voice/VoicePanel";
import { ScreenViewer } from "./components/ScreenShare/ScreenViewer";
import { FileList } from "./components/FileShare/FileList";
import { SettingsPage } from "./components/Settings/SettingsPage";
import { SpaceLogView } from "./components/Log/SpaceLogView";
import { NotFoundPage } from "./components/Common/NotFoundPage";
import { AppLoadingScreen, AppErrorScreen } from "./components/Common/AppLoadingScreen";
import { useSpaceStore } from "./stores/spaceStore";
import { useSettingsStore } from "./stores/settingsStore";
import { useEffect, useState } from "react";
import { Theme } from "@radix-ui/themes";
import { isDaemonReady, getDaemonErrorReason } from "./utils/api";
import { listen } from "@tauri-apps/api/event";
import { initRealtime } from "./services/realtime";
import { applyGlobalShortcuts, handleShortcutPress } from "./services/shortcuts";
import { registerSignalHandler, resolveMember } from "./services/signal";
import * as api from "./utils/api";
import { useFileStore } from "./stores/fileStore";
import type { FileInfo } from "./types";

const POLL_INTERVAL_MS = 1000;
const POLL_MAX_ATTEMPTS = 30;

export default function App() {
  const loadSpaces = useSpaceStore((s) => s.loadSpaces);
  const theme = useSettingsStore((s) => s.theme);
  const [systemDark, setSystemDark] = useState(
    () => window.matchMedia("(prefers-color-scheme: dark)").matches
  );
  const [appReady, setAppReady] = useState(false);
  const [appError, setAppError] = useState("");

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  const appearance: "light" | "dark" =
    theme === "dark" ? "dark" :
    theme === "light" ? "light" :
    systemDark ? "dark" : "light";

  useEffect(() => {
    let attempts = 0;
    let cancelled = false;

    const check = async () => {
      try {
        const ready = await isDaemonReady();
        if (!cancelled && ready) {
          await loadSpaces();
          setAppReady(true);
          return true;
        }
      } catch {
        // invoke 可能失败（daemon 未就绪），继续轮询
      }
      return false;
    };

    const poll = async () => {
      const ok = await check();
      if (ok) return;

      // 监听 daemon-ready 事件，获取后端精确错误信息
      const unlisten = await listen<{ ready: boolean; reason?: string }>(
        "daemon-ready",
        (event) => {
          if (!event.payload.ready) {
            setAppError(`daemon 启动失败: ${event.payload.reason || "未知原因"}`);
          }
        }
      );

      const timerId = setInterval(async () => {
        attempts++;
        if (cancelled) {
          clearInterval(timerId);
          unlisten();
          return;
        }
        if (attempts > POLL_MAX_ATTEMPTS) {
          clearInterval(timerId);
          unlisten();
          // 超时后尝试读后端记录的错误原因
          try {
            const reason = await getDaemonErrorReason();
            setAppError(
              reason 
                ? `daemon 启动超时: ${reason}` 
                : "daemon 启动超时（超过30秒），请重试"
            );
          } catch {
            setAppError("daemon 启动超时（超过30秒），请重试");
          }
          return;
        }
        const ok = await check();
        if (ok) {
          clearInterval(timerId);
          unlisten();
        }
      }, POLL_INTERVAL_MS);
    };

    poll();

    return () => {
      cancelled = true;
    };
  }, []);

  // 初始化实时事件中枢（监听 new_message，分发聊天/信令）
  useEffect(() => {
    if (!appReady) return;
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    initRealtime()
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((e) => console.error("[realtime] init failed:", e));

    applyGlobalShortcuts().catch((e) =>
      console.error("[shortcuts] init failed:", e)
    );

    // Web 模式无全局快捷键插件，降级为页面内 Ctrl+M / Ctrl+T
    const onKeyDown = (e: KeyboardEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      const key = e.key.toLowerCase();
      if (key === "m") handleShortcutPress("Ctrl+M");
      else if (key === "t") handleShortcutPress("Ctrl+T");
    };
    window.addEventListener("keydown", onKeyDown);
    const removeKeyDown = () => window.removeEventListener("keydown", onKeyDown);

    // 文件信令：收到 "sent" 时落库并刷新文件列表
    const unregisterFileSignal = registerSignalHandler("file", async (spaceId, env) => {
      if (env.type !== "sent") return;
      try {
        const data = env.data as { file?: FileInfo };
        const fileInfo = data?.file;
        if (!fileInfo || !fileInfo.id) return;
        await api.recordReceivedFile(fileInfo);
        const fresh = await api.listFiles(spaceId);
        useFileStore.getState().setFiles(spaceId, fresh);
        // 系统通知：收到新文件
        try {
          const { isPermissionGranted, requestPermission, sendNotification } =
            await import("@tauri-apps/plugin-notification");
          let granted = await isPermissionGranted();
          if (!granted) granted = (await requestPermission()) === "granted";
          if (granted) {
            const member = resolveMember(spaceId, env.from);
            sendNotification({
              title: fileInfo.file_name,
              body: member?.nickname || env.from || "",
            });
          }
        } catch (e) {
          console.warn("[file] notification error:", e);
        }
      } catch (e) {
        console.error("[file] record received file failed:", e);
      }
    });

    return () => {
      cancelled = true;
      unregisterFileSignal();
      unlisten?.();
      removeKeyDown();
    };
  }, [appReady]);

  if (!appReady && !appError) {
    return (
      <Theme accentColor="blue" grayColor="slate" radius="medium" appearance={appearance} hasBackground>
        <AppLoadingScreen />
      </Theme>
    );
  }

  if (appError) {
    return (
      <Theme accentColor="blue" grayColor="slate" radius="medium" appearance={appearance} hasBackground>
        <AppErrorScreen message={appError} onRetry={() => { setAppError(""); setAppReady(false); loadSpaces(); }} />
      </Theme>
    );
  }

  return (
    <Theme accentColor="blue" grayColor="slate" radius="medium" appearance={appearance} hasBackground>
      <BrowserRouter>
        <AppLayout>
          <TrayBridge />
          <Routes>
            <Route path="/" element={<SpaceList />} />
            <Route path="/space/:id" element={<SpaceDetail />} />
            <Route path="/space/:id/chat" element={<ChatView />} />
            <Route path="/space/:id/voice" element={<VoicePanel />} />
            <Route path="/space/:id/screen" element={<ScreenViewer />} />
            <Route path="/space/:id/files" element={<FileList />} />
            <Route path="/space/:id/logs" element={<SpaceLogView />} />
            <Route path="/space/:id/app/:appId" element={<AppBrowserView />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="*" element={<NotFoundPage />} />
          </Routes>
        </AppLayout>
      </BrowserRouter>
    </Theme>
  );
}