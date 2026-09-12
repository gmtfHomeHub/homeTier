import { useEffect, useRef } from "react";
import { useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useSpaceStore } from "../../stores/spaceStore";
import { useAppTabsStore } from "../../stores/appTabsStore";
import { usePeerStore } from "../../stores/peerStore";
import * as api from "../../utils/api";

/** 等待空间至少有一个已连接 peer（含本机）且虚拟 IP 已分配，最多等待 30s */
async function waitForPeerReady(spaceId: string): Promise<boolean> {
  const start = Date.now();
  const MAX_WAIT_MS = 30_000;
  const POLL_MS = 500;

  while (Date.now() - start < MAX_WAIT_MS) {
    const peers = usePeerStore.getState().peers[spaceId] ?? [];
    const space = useSpaceStore.getState().spaces.find((sp) => sp.id === spaceId);
    if (peers.length > 0 && space?.virtual_ip) {
      return true;
    }
    await new Promise((r) => setTimeout(r, POLL_MS));
  }
  return false;
}

export function AppBrowserView() {
  const { t } = useTranslation();
  const { id, appId } = useParams<{ id: string; appId: string }>();
  const abortRef = useRef(false);

  // 仅用 id/appId 触发，避免 space 对象引用变化导致 effect 重跑
  useEffect(() => {
    abortRef.current = false;
    if (!id || !appId) return;

    const run = async () => {
      const key = `${id}:${appId}`;
      const { openApps, setActive, openApp } = useAppTabsStore.getState();
      const space = useSpaceStore.getState().spaces.find((sp) => sp.id === id);
      const existing = openApps.find((tab) => tab.key === key);

      if (existing) {
        // 深链直达已打开的标签：仅激活，不重复打开
        setActive(existing.key);
        return;
      }

      // 就绪门控：等待至少一个 peer 连接上（含本机），避免连接建立前打开应用导致连接超时
      const ready = await waitForPeerReady(id);
      if (abortRef.current) return;
      if (!ready) {
        console.warn(`[AppBrowserView] space ${id} 等待 peer 就绪超时，仍尝试打开应用`);
      }

      // 路由直达或刷新：拉取应用后打开标签
      const apps = await api.listApps(id);
      if (abortRef.current) return;
      const found = apps.find((a) => a.id === appId);
      if (found && space) {
        openApp(space, found);
      }
    };

    run();
    return () => {
      abortRef.current = true;
    };
  }, [id, appId]);

  return (
    <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)]">
      {t("common.loading")}
    </div>
  );
}