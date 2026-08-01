import { create } from "zustand";
import { persist } from 'zustand/middleware';
import { ThemeEnum, LanguageEnum } from "../enum";

interface SettingsStore {
  theme: ThemeEnum;
  language: LanguageEnum;
  relayPrefix: string;
  useProxy: boolean;
  logEnabled: boolean;
  micShortcut: string;
  speakerShortcut: string;
  setTheme: (theme: ThemeEnum) => void;
  setLanguage: (lang: LanguageEnum) => void;
  setUseProxy: (v: boolean) => void;
  setLogEnabled: (v: boolean) => void;
  setMicShortcut: (v: string) => void;
  setSpeakerShortcut: (v: string) => void;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set) => ({
      theme: ThemeEnum.SYS,
      language: LanguageEnum.ZH,
      relayPrefix: "homeTier_",
      useProxy: true,
      logEnabled: true,
      micShortcut: "Ctrl+M",
      speakerShortcut: "Ctrl+T",

      setTheme: (theme) => set({ theme }),
      setLanguage: (language) => set({ language }),
      setUseProxy: (useProxy) => set({ useProxy }),
      setLogEnabled: (logEnabled) => set({ logEnabled }),
      setMicShortcut: (micShortcut) => set({ micShortcut }),
      setSpeakerShortcut: (speakerShortcut) => set({ speakerShortcut }),
    }),
    {
      name: 'settings-store',
      partialize: (state) => ({
        theme: state.theme,
        language: state.language,
        useProxy: state.useProxy,
        micShortcut: state.micShortcut,
        speakerShortcut: state.speakerShortcut,
      }),
    }
  )
);