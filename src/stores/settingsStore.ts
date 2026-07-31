import { create } from "zustand";
import { persist } from 'zustand/middleware';
import { ThemeEnum, LanguageEnum } from "../enum";

interface SettingsStore {
  theme: ThemeEnum;
  language: LanguageEnum;
  relayPrefix: string;
  useProxy: boolean;
  setTheme: (theme: ThemeEnum) => void;
  setLanguage: (lang: LanguageEnum) => void;
  setUseProxy: (v: boolean) => void;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set) => ({
      theme: ThemeEnum.SYS,
      language: LanguageEnum.ZH,
      relayPrefix: "homeTier_",
      useProxy: true,

      setTheme: (theme) => set({ theme }),
      setLanguage: (language) => set({ language }),
      setUseProxy: (useProxy) => set({ useProxy }),
    }),
    {
      name: 'settings-store',
      partialize: (state) => ({
        theme: state.theme,
        language: state.language,
        useProxy: state.useProxy,
      }),
    }
  )
);