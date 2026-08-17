import { create } from "zustand";
import * as api from "../utils/api";

interface UpdateStore {
  currentVersion: string | null;
  latestVersion: string | null;
  hasUpdate: boolean;
  checked: boolean;
  checking: boolean;
  checkAppUpdate: () => Promise<void>;
}

export const useUpdateStore = create<UpdateStore>((set) => ({
  currentVersion: null,
  latestVersion: null,
  hasUpdate: false,
  checked: false,
  checking: false,
  checkAppUpdate: async () => {
    set({ checking: true });
    try {
      const res = await api.checkAppUpdate();
      set({
        currentVersion: res.current,
        latestVersion: res.latest,
        hasUpdate: res.has_update,
        checked: true,
      });
    } catch {
      // 检查失败（离线等）静默，不打扰用户
      set({ checked: true });
    } finally {
      set({ checking: false });
    }
  },
}));
