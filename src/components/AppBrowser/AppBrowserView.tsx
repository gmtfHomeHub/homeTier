import { useEffect } from "react";
import { useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useSpaceStore } from "../../stores/spaceStore";
import { useAppTabsStore } from "../../stores/appTabsStore";
import * as api from "../../utils/api";

export function AppBrowserView() {
  const { t } = useTranslation();
  const { id, appId } = useParams<{ id: string; appId: string }>();
  const space = useSpaceStore((s) => (id ? s.spaces.find((sp) => sp.id === id) : undefined));

  useEffect(() => {
    if (!id || !appId || !space) return;
    const key = `${id}:${appId}`;
    const { openApps, setActive, openApp } = useAppTabsStore.getState();
    const existing = openApps.find((tab) => tab.key === key);
    if (existing) {
      // 深链直达已打开的标签：仅激活，不重复打开
      setActive(existing.key);
      return;
    }
    // 路由直达或刷新：拉取应用后打开标签
    api.listApps(id).then((apps) => {
      const found = apps.find((a) => a.id === appId);
      if (found) {
        openApp(space, found);
      }
    });
  }, [id, appId, space]);

  return (
    <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)]">
      {t("common.loading")}
    </div>
  );
}
