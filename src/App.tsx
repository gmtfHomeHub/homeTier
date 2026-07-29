import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppLayout } from "./components/Layout/AppLayout";
import { SpaceList } from "./components/Space/SpaceList";
import { SpaceDetail } from "./components/Space/SpaceDetail";
import { AppBrowserView } from "./components/AppBrowser/AppBrowserView";
import { ChatView } from "./components/Chat/ChatView";
import { VoicePanel } from "./components/Voice/VoicePanel";
import { FileList } from "./components/FileShare/FileList";
import { SettingsPage } from "./components/Settings/SettingsPage";
import { SpaceLogView } from "./components/Log/SpaceLogView";
import { NotFoundPage } from "./components/Common/NotFoundPage";
import { AppLoadingScreen, AppErrorScreen } from "./components/Common/AppLoadingScreen";
import { useSpaceStore } from "./stores/spaceStore";
import { useSettingsStore } from "./stores/settingsStore";
import { useEffect, useState } from "react";
import { Theme } from "@radix-ui/themes";
import { isDaemonReady } from "./utils/api";

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

      const timerId = setInterval(async () => {
        attempts++;
        if (cancelled) {
          clearInterval(timerId);
          return;
        }
        if (attempts > POLL_MAX_ATTEMPTS) {
          clearInterval(timerId);
          setAppError("daemon 启动超时");
          return;
        }
        const ok = await check();
        if (ok) clearInterval(timerId);
      }, POLL_INTERVAL_MS);
    };

    poll();

    return () => {
      cancelled = true;
    };
  }, []);

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
          <Routes>
            <Route path="/" element={<SpaceList />} />
            <Route path="/space/:id" element={<SpaceDetail />} />
            <Route path="/space/:id/chat" element={<ChatView />} />
            <Route path="/space/:id/voice" element={<VoicePanel />} />
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