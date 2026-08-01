import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import type { ShortcutEvent } from "@tauri-apps/plugin-global-shortcut";
import { useSettingsStore } from "../stores/settingsStore";
import { useShortcutOsdStore } from "../stores/shortcutOsdStore";
import { voiceService } from "./voice";

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

function toHandler(shortcut: string): (event: ShortcutEvent) => void {
  return (event) => {
    if (event.state === "Pressed") handlePressed(shortcut);
  };
}

export async function applyGlobalShortcuts(): Promise<void> {
  const { micShortcut, speakerShortcut } = useSettingsStore.getState();

  try {
    await unregister([micShortcut, speakerShortcut]);
  } catch {
    // 未注册或权限不足时忽略
  }

  const all = [micShortcut, speakerShortcut];
  const unique = [...new Set(all)];
  try {
    await register(unique, (event) => {
      if (event.state === "Pressed") handlePressed(event.shortcut);
    });
  } catch (e) {
    console.error("[shortcuts] register failed:", e);
  }
}

export { DEFAULT_MIC_SHORTCUT, DEFAULT_SPEAKER_SHORTCUT };
