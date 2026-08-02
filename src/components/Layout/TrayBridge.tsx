import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { useSpaceStore, resyncTrayMenu } from "../../stores/spaceStore";
import i18n from "../../i18n";

export function TrayBridge() {
  const navigate = useNavigate();
  const spaces = useSpaceStore((s) => s.spaces);

  useEffect(() => {
    const unlisten = listen<string>("tray-navigate", (event) => {
      const spaceId = event.payload;
      const space = spaces.find((s) => s.id === spaceId);
      if (!space) return;
      // 仅切换空间页面，不触发连接；窗口激活由后端（macOS activate_main_window）统一处理
      navigate(`/space/${spaceId}`);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [spaces, navigate]);

  // 语言切换后重刷托盘菜单文案（显示/隐藏、退出）
  useEffect(() => {
    const onLanguageChanged = () => resyncTrayMenu();
    i18n.on("languageChanged", onLanguageChanged);
    return () => {
      i18n.off("languageChanged", onLanguageChanged);
    };
  }, []);

  return null;
}
