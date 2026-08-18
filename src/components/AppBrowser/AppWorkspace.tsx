import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  ArrowLeft,
  RefreshCw,
  ExternalLink,
  Monitor,
  Smartphone,
  X,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { Button, Badge, TextField } from "@radix-ui/themes";
import { listen } from "@tauri-apps/api/event";
import { useAppTabsStore } from "../../stores/appTabsStore";
import { open } from "@tauri-apps/plugin-shell";
import * as api from "../../utils/api";
import { toastInfo } from "../../utils/toast";
import { ProxyFrame, ProxyErrorFallback, sendFrameNavCmd, type FrameNavState } from "./ProxyFrame";

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
  const [navStates, setNavStates] = useState<Record<string, FrameNavState>>({});

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

  // 历史前进/后退：通过注入脚本导航桥控制 iframe 会话栈
  const handleHistoryBack = useCallback(() => {
    if (activeKey) sendFrameNavCmd(activeKey, "back");
  }, [activeKey]);

  const handleHistoryFwd = useCallback(() => {
    if (activeKey) sendFrameNavCmd(activeKey, "forward");
  }, [activeKey]);

  const [addrInput, setAddrInput] = useState("");
  const handleAddrBar = useCallback(
    (value: string) => {
      if (activeKey) sendFrameNavCmd(activeKey, "go", value.trim());
    },
    [activeKey]
  );

  // 设备模式切换：同步后端（UA 注入/移动仿真）并整体刷新以重新注入脚本
  const handleToggleDevice = useCallback(async () => {
    const next = deviceMode === "desktop" ? "mobile" : "desktop";
    try {
      await api.setDeviceMode(next);
    } catch {
      // 后端失败仍切换本地展示
    }
    setDeviceMode(next);
    setRefreshNonce((m) => {
      const n: Record<string, number> = {};
      for (const tab of openApps) n[tab.key] = (m[tab.key] ?? 0) + 1;
      return n;
    });
    setNavStates({});
  }, [deviceMode, setDeviceMode, openApps]);

  // 监听后端下载完成事件，提示文件保存位置（替代轮询，避免误报）
  useEffect(() => {
    if (!visible) return;
    const unlisten = listen<string>("proxy-download", (e) => {
      const path = e.payload ?? "";
      const name = path.split("/").pop() ?? path;
      if (name) toastInfo(`${t("common.downloadSaved")}: ${name}`);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [visible, t]);

  const handleNavState = useCallback((key: string, state: FrameNavState) => {
    setNavStates((m) => ({ ...m, [key]: state }));
  }, []);

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
      <div className="flex items-center gap-1.5 px-3 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        <Button onClick={handleBack} variant="ghost" size="2" title={t("common.back")}>
          <ArrowLeft size={18} />
        </Button>
        <Button onClick={handleRefresh} variant="ghost" size="2" title={t("common.refresh")}>
          <RefreshCw size={18} />
        </Button>
        <Button
          onClick={handleHistoryBack}
          variant="ghost"
          size="2"
          disabled={!(navStates[activeKey ?? ""]?.canBack)}
          title={t("common.historyBack")}
        >
          <ChevronLeft size={18} />
        </Button>
        <Button
          onClick={handleHistoryFwd}
          variant="ghost"
          size="2"
          disabled={!(navStates[activeKey ?? ""]?.canFwd)}
          title={t("common.historyForward")}
        >
          <ChevronRight size={18} />
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
          onClick={handleToggleDevice}
          variant="ghost"
          size="2"
          title={
            deviceMode === "desktop"
              ? t("common.switchToMobile")
              : t("common.switchToDesktop")
          }
        >
          {deviceMode === "desktop" ? <Smartphone size={16} /> : <Monitor size={16} />}
        </Button>
        <Button onClick={handleOpenInBrowser} variant="ghost" size="2" title={t("common.openInBrowser")}>
          <ExternalLink size={16} />
        </Button>
      </div>

      {/* 地址栏（hidden：CSS 隐藏但保留 DOM，供导航桥/未来启用） */}
      {activeTab && (
        <div className="hidden flex items-center gap-2 px-4 py-1.5 border-b border-[var(--color-border)] bg-[var(--color-bg)] shrink-0">
          <TextField.Root
            // value 由导航状态上报驱动，非受控展示
            value={addrInput || navStates[activeTab.key]?.url || activeTab.proxyUrl}
            onChange={(e) => setAddrInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                handleAddrBar(addrInput);
                setAddrInput("");
              }
            }}
            placeholder={t("common.addressBar")}
            className="flex-1"
          >
            <TextField.Slot />
          </TextField.Root>
        </div>
      )}

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
                  tabKey={tab.key}
                  proxyUrl={tab.proxyUrl}
                  name={tab.app.name}
                  deviceMode={deviceMode}
                  onOpenBrowser={handleOpenInBrowser}
                  onBack={handleBack}
                  onError={() => setLoadError(tab.key, true)}
                  onNavState={(s) => handleNavState(tab.key, s)}
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
