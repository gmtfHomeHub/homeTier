import { create } from "zustand";

export type ShortcutOsdKind = "mic" | "speaker";

interface ShortcutOsdState {
  visible: boolean;
  kind: ShortcutOsdKind;
  muted: boolean;
  notJoined: boolean;
  tick: number;
  show: (kind: ShortcutOsdKind, muted: boolean, notJoined?: boolean) => void;
  hide: () => void;
}

let hideTimer: ReturnType<typeof setTimeout> | null = null;

export const useShortcutOsdStore = create<ShortcutOsdState>((set) => ({
  visible: false,
  kind: "mic",
  muted: false,
  notJoined: false,
  tick: 0,
  show: (kind, muted, notJoined = false) => {
    if (hideTimer) clearTimeout(hideTimer);
    set((s) => ({ visible: true, kind, muted, notJoined, tick: s.tick + 1 }));
    hideTimer = setTimeout(() => set({ visible: false }), 1500);
  },
  hide: () => {
    if (hideTimer) clearTimeout(hideTimer);
    set({ visible: false });
  },
}));
