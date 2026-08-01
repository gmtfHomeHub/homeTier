import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { ArrowLeft, RefreshCw, ExternalLink, Monitor, Smartphone, X } from "lucide-react";
import { Button, Badge } from "@radix-ui/themes";
import { useAppTabsStore } from "../../stores/appTabsStore";
import { open } from "@tauri-apps/plugin-shell";
import * as api from "../../utils/api";
import { ProxyFrame, ProxyErrorFallback } from "./ProxyFrame";

export function AppWorkspace() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const openApps = useAppTabsStore((s) => s.openApps);
  const activeKey = useAppTabsStore((s) => s.activeKey);
  const visible = useAppTabsStore((s) => s.visible);
  const setActive = useAppTabsStore((s) => s.setActive);
  const setLoadError = useAppTabsStore((s) => s.setLoadError);
  const closeTab = useAppTabsStore((s) => s.closeTab);
  const hide = useAppTabsStore((s) => s.hide);
  const deviceMode = useAppTabsStore((s) => s.deviceMode);
  const setDeviceMode = useAppTabsStore((s) => s.setDeviceMode);
  const [refreshNonce, setRefreshNonce] = useState<Record<string, number>>({});

  const activeTab = openApps.find((tab) => tab.key === activeKey) ?? null;
  const spaceId = activeTab?.spaceId ?? openApps[0]?.spaceId ?? null;
  const spaceTabs = spaceId ? openApps.filter((tab) => tab.spaceId === spaceId) : [];

  const handleBack = useCallback(() => {
    hide();
    if (spaceId) navigate(`/space/${spaceId}`);
  }, [hide, navigate, spaceId]);

  const handleRefresh = useCallback(() => {
    if (!activeTab) return;
    setLoadError(activeTab.key, false);
    setRefreshNonce((m) => ({ ...m, [activeTab.key]: (m[activeTab.key] ?? 0) + 1 }));
  }, [activeTab, setLoadError]);

  const handleOpenInBrowser = useCallback(async () => {
    if (!activeTab) return;
    try {
      await open(activeTab.appUrl);
    } catch {
      window.open(activeTab.appUrl, "_blank");
    }
  }, [activeTab]);

  const handleSwitchTab = useCallback((key: string) => {
    const tab = openApps.find((x) => x.key === key);
    if (!tab) return;
    setActive(key);
    navigate(`/space/${tab.spaceId}/app/${tab.appId}`);
  }, [openApps, setActive, navigate]);

  const handleCloseTab = useCallback((key: string) => {
    const tab = openApps.find((x) => x.key === key);
    closeTab(key);
    if (key === activeKey) {
      const rest = openApps.filter((x) => x.key !== key && x.spaceId === tab?.spaceId);
      const next = rest.length > 0 ? rest.reduce((a, b) => (b.lastActiveAt > a.lastActiveAt ? b : a)) : null;
      if (next) {
        setActive(next.key);
        navigate(`/space/${next.spaceId}/app/${next.appId}`);
      } else {
        hide();
        navigate(`/space/${tab?.spaceId ?? ""}`);
      }
    }
  }, [activeKey, closeTab, openApps, setActive, navigate, hide]);

  // 没有任何打开的标签时，不渲染（不占位）
  if (openApps.length === 0) {
    return null;
  }

  return (
    <div
      className="absolute inset-0 z-20 bg-[var(--color-bg)] flex flex-col"
      style={visible ? undefined : { display: "none" }}
    >
      {/* 标签栏 */}
      <div className="flex items-center gap-2 px-4 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        <Button onClick={handleBack} variant="ghost" size="2" title={t("common.back")}>
          <ArrowLeft size={18} />
        </Button>
        <Button onClick={handleRefresh} variant="ghost" size="2" title={t("common.refresh")}>
          <RefreshCw size={18} />
        </Button>
        <div className="flex-1 flex items-center gap-1.5 min-w-0 overflow-x-auto">
          {spaceTabs.map((tab) => (
            <div
              key={tab.key}
              onClick={() => handleSwitchTab(tab.key)}
              className={`flex items-center gap-2 pl-3 pr-1.5 py-1 rounded-lg cursor-pointer text-sm whitespace-nowrap transition-colors ${
                tab.key === activeKey
                  ? "bg-[var(--color-primary)]/10 text-[var(--color-primary)]"
                  : "text-[var(--color-text-secondary)] hover:bg-[var(--color-border)]/50"
              }`}
            >
              <span className="max-w-[120px] truncate">{tab.app.name}</span>
              <Badge
                onClick={(e) => {
                  e.stopPropagation();
                  handleCloseTab(tab.key);
                }}
                // variant="soft"
                // size="1"
                className="p-0.125 rounded hover:bg-[var(--color-border)]"
                title={t("common.close")}
              >
                <X size={12} />
              </Badge>
            </div>
          ))}
        </div>
        <Button
          onClick={() => setDeviceMode(deviceMode === "desktop" ? "mobile" : "desktop")}
          variant="ghost"
          size="2"
          title={
            deviceMode === "desktop"
              ? t("common.switchToMobile")
              : t("common.switchToDesktop")
          }
        >
          {deviceMode === "desktop" ? <Monitor size={16} /> : <Smartphone size={16} />}
        </Button>
        <Button onClick={handleOpenInBrowser} variant="ghost" size="2" title={t("common.openInBrowser")}>
          <ExternalLink size={16} />
        </Button>
      </div>

      {/* 内容区：全部 iframe 保持挂载，仅活跃可见 */}
      <div className="relative flex-1 bg-white">
        {spaceTabs.map((tab) => {
          const isActive = tab.key === activeKey;
          const refreshKey = refreshNonce[tab.key] ?? 0;
          const showFrame = tab.proxyUrl && !tab.loadError;
          return (
            <div
              key={tab.key}
              className="absolute inset-0"
              style={{ display: isActive ? "block" : "none" }}
            >
              {showFrame ? (
                <ProxyFrame
                  key={refreshKey}
                  proxyUrl={tab.proxyUrl}
                  name={tab.app.name}
                  deviceMode={deviceMode}
                  onOpenBrowser={handleOpenInBrowser}
                  onBack={handleBack}
                  onError={() => setLoadError(tab.key, true)}
                />
              ) : tab.loadError ? (
                <ProxyErrorFallback onOpenBrowser={handleOpenInBrowser} onBack={handleBack} />
              ) : (
                <div className="absolute inset-0 flex items-center justify-center text-[var(--color-text-secondary)]">
                  {t("common.invalidUrl")}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
