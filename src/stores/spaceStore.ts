import { create } from "zustand";
import * as api from "../utils/api";
import type { Space } from "../types";

interface SpaceStore {
  spaces: Space[];
  currentSpaceId: string | null;

  loadSpaces: () => Promise<void>;
  loadSpacesOnce: () => Promise<void>;
  createSpace: (name: string, networkSecret: string, ownerId: string, description?: string) => Promise<Space>;
  joinSpace: (networkName: string, networkSecret: string) => Promise<Space>;
  leaveSpace: (spaceId: string) => Promise<void>;
  deleteSpace: (spaceId: string, callerId?: string) => Promise<void>;
  removeMember: (spaceId: string, targetMemberId: string, callerId: string) => Promise<void>;
  setCurrentSpace: (id: string | null) => void;
  connectSpace: (spaceId: string) => Promise<void>;
  disconnectSpace: (spaceId: string) => Promise<void>;
  updateSpaceStatus: (spaceId: string, status: Space["status"], virtualIp?: string) => void;
}

export const useSpaceStore = create<SpaceStore>((set, get) => ({
  spaces: [],
  currentSpaceId: null,

  loadSpaces: async () => {
    const spaces = await api.listSpaces();
    set({ spaces });
  },

  loadSpacesOnce: async () => {
    try {
      const spaces = await api.listSpaces();
      set({ spaces });
    } catch (e) {
      // silently ignore
    }
  },

  createSpace: async (name, networkSecret, ownerId, description) => {
    const space = await api.createSpace(name, networkSecret, ownerId, description);
    set((state) => ({ spaces: [...state.spaces, space] }));
    return space;
  },

  joinSpace: async (networkName, networkSecret) => {
    const space = await api.joinSpace(networkName, networkSecret);
    set((state) => ({ spaces: [...state.spaces, space] }));
    return space;
  },

  leaveSpace: async (spaceId) => {
    await api.leaveSpace(spaceId);
    set((state) => ({
      spaces: state.spaces.map((s) =>
        s.id === spaceId ? { ...s, status: "disconnected" as const } : s
      ),
    }));
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
  },

  removeMember: async (spaceId, targetMemberId, callerId) => {
    await api.removeMember(spaceId, targetMemberId, callerId);
    const updated = await api.listMembers(spaceId);
    set((state) => ({
      spaces: state.spaces.map((s) =>
        s.id === spaceId ? { ...s, member_count: updated.length } : s
      ),
    }));
  },

  setCurrentSpace: (id) => set({ currentSpaceId: id }),

  connectSpace: async (spaceId) => {
    // 互斥：将其他已连接的空间设为 disconnected，目标空间设为 connecting
    set((state) => ({
      spaces: state.spaces.map((s) => {
        if (s.id === spaceId) return { ...s, status: "connecting" as const };
        if (s.status === "connected" || s.status === "connecting") return { ...s, status: "disconnected" as const, virtual_ip: undefined };
        return s;
      }),
    }));
    try {
      await api.connectSpace(spaceId);
      set((state) => ({
        spaces: state.spaces.map((s) =>
          s.id === spaceId ? { ...s, status: "connected" as const } : s
        ),
      }));
    } catch (e) {
      set((state) => ({
        spaces: state.spaces.map((s) =>
          s.id === spaceId ? { ...s, status: "disconnected" as const } : s
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
        s.id === spaceId ? { ...s, status: "disconnected", virtual_ip: undefined } : s
      ),
    }));
  },

  updateSpaceStatus: (spaceId, status, virtualIp) => {
    set((state) => ({
      spaces: state.spaces.map((s) =>
        s.id === spaceId ? { ...s, status, virtual_ip: virtualIp ?? s.virtual_ip } : s
      ),
    }));
  },
}));