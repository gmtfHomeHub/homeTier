import { create } from "zustand";
import type { Space, SpaceApp } from "../types";
import { buildAppUrl } from "../types";
import { buildProxyUrl, resolveProxyUrl } from "../components/AppBrowser/ProxyFrame";
import { detectDeviceMode, type DeviceMode } from "../utils/device";

export const MAX_IFRAMES = 10;

export interface AppTab {
  key: string;
  spaceId: string;
  appId: string;
  app: SpaceApp;
  appUrl: string;
  proxyUrl: string;
  engine: "local-http" | "hometierproxy";
  lastActiveAt: number;
  loadError: boolean;
}

interface AppTabsStore {
  openApps: AppTab[];
  activeKey: string | null;
  visible: boolean;
  deviceMode: DeviceMode;

  openApp: (space: Space, app: SpaceApp) => void;
  setActive: (key: string) => void;
  setLoadError: (key: string, err: boolean) => void;
  updateProxyUrl: (key: string, url: string, engine: AppTab["engine"]) => void;
  closeTab: (key: string) => void;
  clearSpace: (spaceId: string) => void;
  hide: () => void;
  setDeviceMode: (mode: DeviceMode) => void;
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
    engine: "hometierproxy",
    lastActiveAt: Date.now(),
    loadError: false,
  };
}

/** 异步解析代理 URL（localHttp 优先），完成后回写 store */
async function resolveTabProxyUrl(key: string, appUrl: string) {
  try {
    const { url, engine } = await resolveProxyUrl(appUrl);
    useAppTabsStore.getState().updateProxyUrl(key, url, engine);
  } catch {
    // 解析失败保留 hometierproxy 回退值
  }
}

export const useAppTabsStore = create<AppTabsStore>((set) => ({
  openApps: [],
  activeKey: null,
  visible: false,
  deviceMode: detectDeviceMode(),

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
      const appUrl = buildAppUrl(app);
      resolveTabProxyUrl(key, appUrl);
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

  updateProxyUrl: (key, url, engine) => {
    set((state) => ({
      openApps: state.openApps.map((t) =>
        t.key === key ? { ...t, proxyUrl: url, engine } : t
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

  setDeviceMode: (mode) => {
    set({ deviceMode: mode });
  },
}));
