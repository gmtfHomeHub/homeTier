import { create } from "zustand";
import * as api from "../utils/api";
import { useAppTabsStore } from "./appTabsStore";
import type { Space } from "../types";
import { SpaceStatus } from "../enum";

interface SpaceStore {
  spaces: Space[];
  currentSpaceId: string | null;

  loadSpaces: () => Promise<void>;
  loadSpacesOnce: () => Promise<void>;
  createSpace: (name: string, networkSecret: string, ownerId: string, description?: string) => Promise<Space>;
  joinSpace: (configJson: string) => Promise<Space>;
  leaveSpace: (spaceId: string) => Promise<void>;
  deleteSpace: (spaceId: string, callerId?: string) => Promise<void>;
  setCurrentSpace: (id: string | null) => void;
  connectSpace: (spaceId: string) => Promise<void>;
  disconnectSpace: (spaceId: string) => Promise<void>;
  updateSpaceStatus: (spaceId: string, status: Space["status"], virtualIp?: string) => void;
}

function syncTrayMenu(spaces: Space[]) {
  api.syncTrayMenu(spaces.map((s) => ({ id: s.id, name: s.name }))).catch(() => {
    // 静默失败，托盘菜单同步失败不影响主流程
  });
}

export const useSpaceStore = create<SpaceStore>((set, get) => ({
  spaces: [],
  currentSpaceId: null,

  loadSpaces: async () => {
    const spaces = await api.listSpaces();
    set({ spaces });
    syncTrayMenu(spaces);
  },

  loadSpacesOnce: async () => {
    try {
      const spaces = await api.listSpaces();
      set({ spaces });
      syncTrayMenu(spaces);
    } catch (e) {
      // silently ignore
    }
  },

  createSpace: async (name, networkSecret, ownerId, description) => {
    const space = await api.createSpace(name, networkSecret, ownerId, description);
    set((state) => ({ spaces: [...state.spaces, space] }));
    syncTrayMenu(get().spaces);
    return space;
  },

  joinSpace: async (configJson) => {
    const space = await api.joinSpace(configJson);
    set((state) => ({ spaces: [...state.spaces, space] }));
    syncTrayMenu(get().spaces);
    return space;
  },

  leaveSpace: async (spaceId) => {
    await api.leaveSpace(spaceId);
    set((state) => ({
      spaces: state.spaces.map((s) =>
        s.id === spaceId ? { ...s, status: SpaceStatus.DIS } : s
      ),
    }));
    syncTrayMenu(get().spaces);
  },

  deleteSpace: async (spaceId, callerId?: string) => {
    if (callerId) {
      await api.deleteSpace(spaceId, callerId);
    } else {
      await api.deleteSpace(spaceId, "");
    }
    set((state) => ({
      spaces: state.spaces.filter((s) => s.id !== spaceId),
      currentSpaceId: state.currentSpaceId === spaceId ? null : state.currentSpaceId,
    }));
    syncTrayMenu(get().spaces);
  },

  setCurrentSpace: (id) => set({ currentSpaceId: id }),

  connectSpace: async (spaceId) => {
    // 互斥：将其他已连接的空间设为 disconnected，目标空间设为 connecting
    const prevConnected = get().spaces.find((s) => s.status === SpaceStatus.CED || s.status === SpaceStatus.ING);
    set((state) => ({
      spaces: state.spaces.map((s) => {
        if (s.id === spaceId) return { ...s, status: SpaceStatus.ING };
        if (s.status === SpaceStatus.CED || s.status === SpaceStatus.ING) return { ...s, status: SpaceStatus.DIS, virtual_ip: undefined };
        return s;
      }),
    }));
    try {
      await api.connectSpace(spaceId);
      set((state) => ({
        spaces: state.spaces.map((s) =>
          s.id === spaceId ? { ...s, status: SpaceStatus.CED } : s
        ),
      }));
      // 空间互斥：清空上一个已连接空间的打开标签
      if (prevConnected && prevConnected.id !== spaceId) {
        useAppTabsStore.getState().clearSpace(prevConnected.id);
      }
      syncTrayMenu(get().spaces);
    } catch (e) {
      set((state) => ({
        spaces: state.spaces.map((s) =>
          s.id === spaceId ? { ...s, status: SpaceStatus.DIS } : s
        ),
        error: String(e),
      }));
      throw e; // 重新抛出，让调用方也能捕获
    }
  },

  disconnectSpace: async (spaceId) => {
    await api.disconnectSpace(spaceId);
    set((state) => ({
      spaces: state.spaces.map((s) =>
        s.id === spaceId ? { ...s, status: SpaceStatus.DIS, virtual_ip: undefined } : s
      ),
    }));
    useAppTabsStore.getState().clearSpace(spaceId);
    syncTrayMenu(get().spaces);
  },

  updateSpaceStatus: (spaceId, status, virtualIp) => {
    set((state) => ({
      spaces: state.spaces.map((s) =>
        s.id === spaceId ? { ...s, status, virtual_ip: virtualIp ?? s.virtual_ip } : s
      ),
    }));
  },
}));