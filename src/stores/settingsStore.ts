import { create } from "zustand";
import { persist } from 'zustand/middleware';
import { ThemeEnum, LanguageEnum, SettingTabEnum } from "../enum";

interface SettingsStore {
  theme: ThemeEnum;
  language: LanguageEnum;
  settingsTab: SettingTabEnum;
  adaptiveLevel: number;
  useProxy: boolean;
  logEnabled: boolean;
  configEnabled: boolean;
  micShortcut: string;
  speakerShortcut: string;
  shortcutEditing: boolean;
  setTheme: (theme: ThemeEnum) => void;
  setLanguage: (lang: LanguageEnum) => void;
  setUseProxy: (v: boolean) => void;
  setLogEnabled: (v: boolean) => void;
  setConfigEnabled: (v: boolean) => void;
  setMicShortcut: (v: string) => void;
  setSpeakerShortcut: (v: string) => void;
  setShortcutEditing: (v: boolean) => void;
  setSettingsTab: (tab: SettingTabEnum) => void;
  setAdaptiveLevel: (v: number) => void;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set) => ({
      theme: ThemeEnum.SYS,
      language: LanguageEnum.ZH,
      useProxy: true,
      logEnabled: true,
      configEnabled: false,
      micShortcut: "Ctrl+M",
      speakerShortcut: "Ctrl+T",
      shortcutEditing: false,
      settingsTab: SettingTabEnum.BASIC,
      adaptiveLevel: 2,
      setTheme: (theme) => set({ theme }),
      setLanguage: (language) => set({ language }),
      setUseProxy: (useProxy) => set({ useProxy }),
      setLogEnabled: (logEnabled) => set({ logEnabled }),
      setConfigEnabled: (configEnabled) => set({ configEnabled }),
      setMicShortcut: (micShortcut) => set({ micShortcut }),
      setSpeakerShortcut: (speakerShortcut) => set({ speakerShortcut }),
      setShortcutEditing: (shortcutEditing) => set({ shortcutEditing }),
      setSettingsTab: (settingsTab) => set({ settingsTab }),
      setAdaptiveLevel: (adaptiveLevel) => set({ adaptiveLevel }),
    }),
    {
      name: 'settings-store',
      partialize: (state) => ({
        theme: state.theme,
        language: state.language,
        useProxy: state.useProxy,
        micShortcut: state.micShortcut,
        speakerShortcut: state.speakerShortcut,
        settingsTab: state.settingsTab,
        adaptiveLevel: state.adaptiveLevel,
        configEnabled: state.configEnabled,
      }),
    }
  )
);