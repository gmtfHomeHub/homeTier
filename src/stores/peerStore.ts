import { create } from "zustand";
import { getSpacePeers } from "../utils/api";
import type { PeerInfo } from "../types";

interface PeerStore {
  /** key: spaceId, value: PeerInfo[] */
  peers: Record<string, PeerInfo[]>;
  /** 记录每个 spaceId 的轮询定时器 handle */
  pollHandles: Record<string, ReturnType<typeof setInterval>>;

  /** 立即拉取一次 peer 列表 */
  fetchPeers: (spaceId: string) => Promise<void>;
  /** 开始轮询（2s 间隔，沿用 NetworkStatsPanel 现有模式） */
  startPolling: (spaceId: string) => void;
  /** 停止轮询并清理数据 */
  stopPolling: (spaceId: string) => void;
  /** 清理某空间数据（不停止轮询时可用） */
  clearPeers: (spaceId: string) => void;
}

const POLL_INTERVAL_MS = 2000; // 与 NetworkStatsPanel 保持一致

export const usePeerStore = create<PeerStore>((set, get) => ({
  peers: {},
  pollHandles: {},

  fetchPeers: async (spaceId: string) => {
    try {
      const list = await getSpacePeers(spaceId);
      set((state) => ({
        peers: { ...state.peers, [spaceId]: list },
      }));
    } catch (e) {
      console.error(`fetchPeers failed for ${spaceId}:`, e);
    }
  },

  startPolling: (spaceId: string) => {
    const { pollHandles } = get();
    if (pollHandles[spaceId]) return; // 已在轮询

    // 立即拉取一次
    get().fetchPeers(spaceId);

    const handle = setInterval(() => {
      get().fetchPeers(spaceId);
    }, POLL_INTERVAL_MS);

    set((state) => ({
      pollHandles: { ...state.pollHandles, [spaceId]: handle },
    }));
  },

  stopPolling: (spaceId: string) => {
    const { pollHandles } = get();
    const handle = pollHandles[spaceId];
    if (handle) {
      clearInterval(handle);
      set((state) => {
        const { [spaceId]: removed, ...rest } = state.pollHandles;
        return { pollHandles: rest };
      });
    }
  },

  clearPeers: (spaceId: string) => {
    set((state) => {
      const { [spaceId]: removed, ...rest } = state.peers;
      return { peers: rest };
    });
  },
}));