import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSpaceStore } from "../../stores/spaceStore";

export function TrayBridge() {
  const navigate = useNavigate();
  const spaces = useSpaceStore((s) => s.spaces);
  const connectSpace = useSpaceStore((s) => s.connectSpace);

  useEffect(() => {
    const unlisten = listen<string>("tray-navigate", (event) => {
      const spaceId = event.payload;
      const space = spaces.find((s) => s.id === spaceId);
      if (!space) return;
      getCurrentWindow().show();
      getCurrentWindow().setFocus();
      if (space.status !== "connected" && space.status !== "connecting") {
        connectSpace(spaceId).catch(() => { /* 连接失败已在 store 中提示 */ });
      }
      navigate(`/space/${spaceId}`);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [spaces, navigate, connectSpace]);

  return null;
}
