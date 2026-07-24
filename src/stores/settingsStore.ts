import { create } from "zustand";
import { persist } from 'zustand/middleware';

interface SettingsStore {
  theme: "light" | "dark" | "system";
  language: "zh" | "zh-TW" | "en";
  autoConnect: boolean;
  minimizeToTray: boolean;
  logLevel: "debug" | "info" | "warn" | "error";
  relayPrefix: string;

  setTheme: (theme: "light" | "dark" | "system") => void;
  setLanguage: (lang: "zh" | "zh-TW" | "en") => void;
  setAutoConnect: (v: boolean) => void;
  setMinimizeToTray: (v: boolean) => void;
  setLogLevel: (level: "debug" | "info" | "warn" | "error") => void;
  setRelayPrefix: (prefix: string) => void;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set) => ({
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
    }),
    {
      name: 'settings-store',
      partialize: (state) => ({
        theme: state.theme,
        language: state.language,
        autoConnect: state.autoConnect,
        minimizeToTray: state.minimizeToTray,
        logLevel: state.logLevel,
        relayPrefix: state.relayPrefix,
      }),
    }
  )
);