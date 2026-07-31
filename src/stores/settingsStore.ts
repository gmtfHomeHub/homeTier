import { create } from "zustand";
import { persist } from 'zustand/middleware';

export type SettingsTab = "basic" | "logs" | "easytier";

interface SettingsStore {
  theme: "light" | "dark" | "system";
  language: "zh" | "zh-TW" | "en";
  relayPrefix: string;
  useProxy: boolean;
  settingsTab: SettingsTab;

  setTheme: (theme: "light" | "dark" | "system") => void;
  setLanguage: (lang: "zh" | "zh-TW" | "en") => void;
  setRelayPrefix: (prefix: string) => void;
  setUseProxy: (v: boolean) => void;
  setSettingsTab: (tab: SettingsTab) => void;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set) => ({
      theme: "system",
      language: "zh",
      relayPrefix: "homeTier_",
      useProxy: true,
      settingsTab: "basic",

      setTheme: (theme) => set({ theme }),
      setLanguage: (language) => set({ language }),
      setRelayPrefix: (relayPrefix) => set({ relayPrefix }),
      setUseProxy: (useProxy) => set({ useProxy }),
      setSettingsTab: (settingsTab) => set({ settingsTab }),
    }),
    {
      name: 'settings-store',
      partialize: (state) => ({
        theme: state.theme,
        language: state.language,
        relayPrefix: state.relayPrefix,
        useProxy: state.useProxy,
        settingsTab: state.settingsTab,
      }),
    }
  )
);