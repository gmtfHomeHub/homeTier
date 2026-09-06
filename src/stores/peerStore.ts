import { create } from "zustand";
import { getSpacePeers, getNetworkStats } from "../utils/api";
import type { PeerInfo, NetworkStats } from "../types";

interface NetworkStatsState {
  rx_bytes: number;
  tx_bytes: number;
  avg_latency_ms: number;
}

interface PeerStore {
  /** key: spaceId, value: PeerInfo[] */
  peers: Record<string, PeerInfo[]>;
  /** key: spaceId, value: NetworkStatsState */
  stats: Record<string, NetworkStatsState>;
  /** 记录每个 spaceId 的轮询定时器 handle */
  pollHandles: Record<string, ReturnType<typeof setInterval>>;

  /** 立即拉取一次 peer 列表 + 网络统计（统一入口） */
  fetchPeers: (spaceId: string) => Promise<void>;
  /** 开始轮询（2s 间隔，统一拉取 peers + stats） */
  startPolling: (spaceId: string) => void;
  /** 停止轮询并清理数据（peers + stats） */
  stopPolling: (spaceId: string) => void;
  /** 清理某空间数据（不停止轮询时可用） */
  clearPeers: (spaceId: string) => void;
}

const POLL_INTERVAL_MS = 2000;

export const usePeerStore = create<PeerStore>((set, get) => ({
  peers: {},
  stats: {},
  pollHandles: {},

  fetchPeers: async (spaceId: string) => {
    try {
      const [peers, netStats] = await Promise.all([
        getSpacePeers(spaceId),
        getNetworkStats(spaceId),
      ]);
      set((state) => ({
        peers: { ...state.peers, [spaceId]: peers },
        stats: { ...state.stats, [spaceId]: {
          rx_bytes: netStats.rx_bytes,
          tx_bytes: netStats.tx_bytes,
          avg_latency_ms: netStats.avg_latency_ms,
        } },
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
      const { [spaceId]: removedPeers, ...peersRest } = state.peers;
      const { [spaceId]: removedStats, ...statsRest } = state.stats;
      return { peers: peersRest, stats: statsRest };
    });
  },
}));