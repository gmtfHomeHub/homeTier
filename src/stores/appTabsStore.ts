import { create } from "zustand";
import type { Space, SpaceApp } from "../types";
import { buildAppUrl } from "../types";
import { buildProxyUrl } from "../components/AppBrowser/ProxyFrame";

export const MAX_IFRAMES = 10;

export interface AppTab {
  key: string;
  spaceId: string;
  appId: string;
  app: SpaceApp;
  appUrl: string;
  proxyUrl: string;
  lastActiveAt: number;
  loadError: boolean;
}

interface AppTabsStore {
  openApps: AppTab[];
  activeKey: string | null;
  visible: boolean;

  openApp: (space: Space, app: SpaceApp) => void;
  setActive: (key: string) => void;
  setLoadError: (key: string, err: boolean) => void;
  closeTab: (key: string) => void;
  clearSpace: (spaceId: string) => void;
  hide: () => void;
}

function makeTab(space: Space, app: SpaceApp): AppTab {
  const appUrl = buildAppUrl(app);
  return {
    key: `${space.id}:${app.id}`,
    spaceId: space.id,
    appId: app.id,
    app,
    appUrl,
    proxyUrl: buildProxyUrl(appUrl),
    lastActiveAt: Date.now(),
    loadError: false,
  };
}

export const useAppTabsStore = create<AppTabsStore>((set) => ({
  openApps: [],
  activeKey: null,
  visible: false,

  openApp: (space, app) => {
    set((state) => {
      const key = `${space.id}:${app.id}`;
      const existing = state.openApps.find((t) => t.key === key);
      if (existing) {
        return {
          openApps: state.openApps.map((t) =>
            t.key === key ? { ...t, lastActiveAt: Date.now() } : t
          ),
          activeKey: key,
          visible: true,
        };
      }

      let openApps = [...state.openApps, makeTab(space, app)];
      if (openApps.length > MAX_IFRAMES) {
        // 淘汰最久未使用的标签
        const oldest = [...openApps].sort((a, b) => a.lastActiveAt - b.lastActiveAt)[0];
        openApps = openApps.filter((t) => t.key !== oldest.key);
      }
      return { openApps, activeKey: key, visible: true };
    });
  },

  setActive: (key) => {
    set((state) => ({
      openApps: state.openApps.map((t) =>
        t.key === key ? { ...t, lastActiveAt: Date.now() } : t
      ),
      activeKey: key,
      visible: true,
    }));
  },

  setLoadError: (key, err) => {
    set((state) => ({
      openApps: state.openApps.map((t) =>
        t.key === key ? { ...t, loadError: err } : t
      ),
    }));
  },

  closeTab: (key) => {
    set((state) => {
      const openApps = state.openApps.filter((t) => t.key !== key);
      let activeKey = state.activeKey;
      if (state.activeKey === key) {
        const rest = [...openApps].sort((a, b) => b.lastActiveAt - a.lastActiveAt);
        activeKey = rest[0]?.key ?? null;
      }
      return { openApps, activeKey };
    });
  },

  clearSpace: (spaceId) => {
    set((state) => {
      const openApps = state.openApps.filter((t) => t.spaceId !== spaceId);
      let activeKey = state.activeKey;
      let visible = state.visible;
      if (activeKey && !openApps.some((t) => t.key === activeKey)) {
        activeKey = null;
        visible = false;
      }
      return { openApps, activeKey, visible };
    });
  },

  hide: () => {
    set({ visible: false });
  },
}));
