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
import { useSpaceStore } from "./stores/spaceStore";
import { useSettingsStore } from "./stores/settingsStore";
import { useEffect, useState } from "react";
import { Theme } from "@radix-ui/themes";

export default function App() {
  const loadSpaces = useSpaceStore((s) => s.loadSpaces);
  const theme = useSettingsStore((s) => s.theme);
  const [systemDark, setSystemDark] = useState(
    () => window.matchMedia("(prefers-color-scheme: dark)").matches
  );

  // 监听系统主题变化
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  // 根据设置解析实际外观值
  const appearance: "light" | "dark" =
    theme === "dark" ? "dark" :
    theme === "light" ? "light" :
    systemDark ? "dark" : "light";

  useEffect(() => {
    loadSpaces();
  }, [loadSpaces]);

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
          </Routes>
        </AppLayout>
      </BrowserRouter>
    </Theme>
  );
}