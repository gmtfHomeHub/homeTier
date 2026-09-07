import { create } from "zustand";
import * as api from "../utils/api";
import { useAppTabsStore } from "./appTabsStore";
import type { Space } from "../types";
import { SpaceStatus } from "../enum";
import i18n from "../i18n";
import { isMobile } from "../utils/platform";
import { connectWithVpn, disconnectWithVpn, getVpnStatus } from "../services/mobileVpn";

interface SpaceStore {
  spaces: Space[];
  currentSpaceId: string | null;

  loadSpaces: () => Promise<void>;
  loadSpacesOnce: () => Promise<void>;
  createSpace: (name: string, networkSecret: string, description?: string) => Promise<Space>;
  joinSpace: (configJson: string, name?: string) => Promise<Space>;
  leaveSpace: (spaceId: string) => Promise<void>;
  deleteSpace: (spaceId: string) => Promise<void>;
  setCurrentSpace: (id: string | null) => void;
  connectSpace: (spaceId: string) => Promise<void>;
  disconnectSpace: (spaceId: string) => Promise<void>;
  updateSpaceStatus: (spaceId: string, status: Space["status"], virtualIp?: string) => void;
}

function syncTrayMenu(spaces: Space[]) {
  api.syncTrayMenu(
    spaces.map((s) => ({ id: s.id, name: s.name })),
    { show: i18n.t("tray.show"), quit: i18n.t("tray.quit") }
  ).catch(() => {
    // 静默失败，托盘菜单同步失败不影响主流程
  });
}

/** 语言变化等场景下，用当前 spaces 重新同步托盘菜单文案 */
export function resyncTrayMenu() {
  syncTrayMenu(useSpaceStore.getState().spaces);
}

export const useSpaceStore = create<SpaceStore>((set, get) => ({
  spaces: [],
  currentSpaceId: null,

  loadSpaces: async () => {
    const prev = get().spaces;
    const spaces = await api.listSpaces();

    // 移动端：若前次 CED 现变 DIS，优先查 VPN 实时状态（Kotlin VpnService 是否运行）
    // 避免 list() 瞬态 DIS 覆盖真实连接状态（Rust is_running() 与 Kotlin VpnService 两层可能不同步）
    const mobile = await isMobile();
    let finalSpaces = spaces;
    if (mobile) {
      const disSpaces = spaces.filter((s) => s.status === SpaceStatus.DIS);
      if (disSpaces.length > 0) {
        try {
          const vpnStatus = await getVpnStatus();
          if (vpnStatus.running) {
            // VPN 实际运行中，恢复这些空间为 CED
            finalSpaces = spaces.map((s) =>
              s.status === SpaceStatus.DIS ? { ...s, status: SpaceStatus.CED } : s
            );
          }
        } catch {
          // getVpnStatus 失败，保持 list() 结果
        }
      }
    }

    set({ spaces: finalSpaces });
    syncTrayMenu(finalSpaces);

    // 异步复核：前次 CED 现变 DIS 的空间（且 VPN 未运行），可能是 list() 瞬态，2s 后重新查询
    const transient = prev.filter(
      (p) =>
        p.status === SpaceStatus.CED &&
        finalSpaces.find((s) => s.id === p.id && s.status === SpaceStatus.DIS)
    );
    if (transient.length > 0) {
      setTimeout(async () => {
        try {
          const rechecked = await api.listSpaces();
          set({ spaces: rechecked });
          syncTrayMenu(rechecked);
        } catch {
          // 静默失败
        }
      }, 2000);
    }
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

  createSpace: async (name, networkSecret, description) => {
    const space = await api.createSpace(name, networkSecret, description);
    set((state) => ({ spaces: [...state.spaces, space] }));
    syncTrayMenu(get().spaces);
    return space;
  },

  joinSpace: async (configJson, name) => {
    const space = await api.joinSpace(configJson, name);
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

  deleteSpace: async (spaceId: string) => {
    await api.deleteSpace(spaceId);
    set((state) => ({
      spaces: state.spaces.filter((s) => s.id !== spaceId),
      currentSpaceId: state.currentSpaceId === spaceId ? null : state.currentSpaceId,
    }));
    syncTrayMenu(get().spaces);
  },

  setCurrentSpace: (id) => set({ currentSpaceId: id }),

  connectSpace: async (spaceId) => {
    // 幂等：若已连接（CED），直接返回，避免重复连接触发 "Invalid IP addr string"
    if (get().spaces.find((s) => s.id === spaceId && s.status === SpaceStatus.CED)) {
      return;
    }

    const mobile = await isMobile();

    // 互斥：将其他已连接的空间设为 disconnected，目标空间设为 connecting
    const prevConnected = get().spaces.find((s) => s.status === SpaceStatus.CED || s.status === SpaceStatus.ING);
    set((state) => ({
      spaces: state.spaces.map((s) => {
        if (s.id === spaceId) return { ...s, status: SpaceStatus.ING };
        if (s.status === SpaceStatus.CED || s.status === SpaceStatus.ING) return { ...s, status: SpaceStatus.DIS, virtual_ip: undefined };
        return s;
      }),
    }));

    if (mobile) {
      try {
        // 移动端：prepareVpn -> connectSpace -> startVpn (事件驱动 setTunFd)
        const space = get().spaces.find((s) => s.id === spaceId);
        if (!space) throw new Error("Space not found");

        // 空串/null/非法 IPv4 均回退默认 IP，防止传空给 startVpn 的 ipv4Addr 触发 "Invalid IP addr string"
        const ipv4Regex = /^\d{1,3}(\.\d{1,3}){3}$/;
        const virtualIp = space.virtual_ip && space.virtual_ip.trim() && ipv4Regex.test(space.virtual_ip.trim())
          ? space.virtual_ip.trim()
          : "10.144.144.1";
        const errorMsg = await connectWithVpn(spaceId, space.name, virtualIp);
        if (errorMsg) {
          throw new Error(errorMsg);
        }

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
        return;
      } catch (e) {
        set((state) => ({
          spaces: state.spaces.map((s) =>
            s.id === spaceId ? { ...s, status: SpaceStatus.DIS } : s
          ),
          error: String(e),
        }));
        // 统一由调用方（useSpaceConnect）toast 一次，避免与 vpn.connectFailed 重复弹两条
        throw e;
      }
    }

    // 桌面端原逻辑
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
    const mobile = await isMobile();
    if (mobile) {
      try {
        await disconnectWithVpn(spaceId);
      } catch (e) {
        console.error("Mobile VPN disconnect failed:", e);
      }
    } else {
      await api.disconnectSpace(spaceId);
    }
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