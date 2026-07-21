import { create } from "zustand";

interface SettingsStore {
  theme: "light" | "dark" | "system";
  language: "zh" | "zh-TW" | "en";
  autoConnect: boolean;
  minimizeToTray: boolean;
  logLevel: string;
  relayPrefix: string;

  setTheme: (theme: "light" | "dark" | "system") => void;
  setLanguage: (lang: "zh" | "zh-TW" | "en") => void;
  setAutoConnect: (v: boolean) => void;
  setMinimizeToTray: (v: boolean) => void;
  setLogLevel: (level: string) => void;
  setRelayPrefix: (prefix: string) => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  theme: "system",
  language: "zh",
  autoConnect: false,
  minimizeToTray: true,
  logLevel: "info",
  relayPrefix: "homeTier_",

  setTheme: (theme) => set({ theme }),
  setLanguage: (language) => set({ language }),
  setAutoConnect: (autoConnect) => set({ autoConnect }),
  setMinimizeToTray: (minimizeToTray) => set({ minimizeToTray }),
  setLogLevel: (logLevel) => set({ logLevel }),
  setRelayPrefix: (relayPrefix) => set({ relayPrefix }),
}));