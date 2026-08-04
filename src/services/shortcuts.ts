import { useSettingsStore } from "../stores/settingsStore";
import { useShortcutOsdStore } from "../stores/shortcutOsdStore";
import { voiceService } from "./voice";
import { isTauri } from "../utils/api";

const DEFAULT_MIC_SHORTCUT = "Ctrl+M";
const DEFAULT_SPEAKER_SHORTCUT = "Ctrl+T";

function handlePressed(shortcut: string): void {
  const { micShortcut, speakerShortcut } = useSettingsStore.getState();
  if (!voiceService.joined) {
    useShortcutOsdStore.getState().show("mic", true, true);
    return;
  }
  if (shortcut === micShortcut) {
    void voiceService.toggleMic().then((muted) => {
      useShortcutOsdStore.getState().show("mic", muted);
    });
  } else if (shortcut === speakerShortcut) {
    void voiceService.toggleSpeaker().then((muted) => {
      useShortcutOsdStore.getState().show("speaker", muted);
    });
  }
}

function toHandler(shortcut: string): (event: { state: string }) => void {
  return (event) => {
    if (event.state === "Pressed") handlePressed(shortcut);
  };
}

export async function applyGlobalShortcuts(): Promise<void> {
  // Web 模式无全局快捷键插件，仅保留页面内快捷入口
  if (!isTauri()) return;
  const { micShortcut, speakerShortcut } = useSettingsStore.getState();
  let mod: typeof import("@tauri-apps/plugin-global-shortcut");
  try {
    mod = await import("@tauri-apps/plugin-global-shortcut");
  } catch {
    // 插件未注册时忽略
    return;
  }

  try {
    await mod.unregister([micShortcut, speakerShortcut]);
  } catch {
    // 未注册或权限不足时忽略
  }

  const all = [micShortcut, speakerShortcut];
  const unique = [...new Set(all)];
  try {
    await mod.register(unique, (event) => {
      if (event.state === "Pressed") handlePressed(event.shortcut);
    });
  } catch (e) {
    console.error("[shortcuts] register failed:", e);
  }
}

export { DEFAULT_MIC_SHORTCUT, DEFAULT_SPEAKER_SHORTCUT };
export { handlePressed as handleShortcutPress };
