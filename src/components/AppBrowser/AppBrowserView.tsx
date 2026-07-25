import { useState, useEffect, useCallback } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { ArrowLeft, RefreshCw, ExternalLink, Loader2, Monitor } from "lucide-react";
import { Button, Flex, Text } from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { open } from "@tauri-apps/plugin-shell";
import * as api from "../../utils/api";
import { buildAppUrl } from "../../types";
import type { SpaceApp } from "../../types";
import { ProxyFrame, buildProxyUrl, ProxyErrorFallback } from "./ProxyFrame";

export function AppBrowserView() {
  const { t } = useTranslation();
  const { id, appId } = useParams<{ id: string; appId: string }>();
  const navigate = useNavigate();
  const { spaces } = useSpaceStore();
  const [app, setApp] = useState<SpaceApp | null>(null);
  const [loading, setLoading] = useState(true);
  const [iframeKey, setIframeKey] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [webappMode, setWebappMode] = useState<string>("iframe");
  const [webviewReady, setWebviewReady] = useState(false);

  const space = spaces.find((s) => s.id === id);

  useEffect(() => {
    if (id && appId) {
      api.listApps(id).then((apps) => {
        const found = apps.find((a) => a.id === appId);
        setApp(found || null);
        setLoading(false);
      });
    }
  }, [id, appId]);

  useEffect(() => {
    api.getWebappMode().then(setWebappMode);
  }, []);

  const appUrl = app ? buildAppUrl(app) : null;
  const proxyAppUrl = appUrl ? buildProxyUrl(appUrl) : null;

  // webview mode
  useEffect(() => {
    if (webappMode !== "webview" || !proxyAppUrl) return;
    api.openAppView(proxyAppUrl, 0, 56, window.innerWidth, window.innerHeight - 56)
      .then(() => setWebviewReady(true))
      .catch(console.error);
  }, [webappMode, proxyAppUrl]);

  useEffect(() => {
    if (webappMode !== "webview") return;
    const handler = () => {
      api.resizeAppView(0, 56, window.innerWidth, window.innerHeight - 56)
        .catch(console.error);
    };
    window.addEventListener("resize", handler);
    return () => window.removeEventListener("resize", handler);
  }, [webappMode]);

  useEffect(() => {
    return () => {
      api.closeAppView().catch(console.error);
    };
  }, []);

  const handleRefresh = useCallback(() => {
    setLoadError(false);
    if (webappMode === "webview" && proxyAppUrl) {
      api.closeAppView().then(() => {
        setWebviewReady(false);
        return api.openAppView(proxyAppUrl, 0, 56, window.innerWidth, window.innerHeight - 56);
      }).then(() => setWebviewReady(true)).catch(console.error);
    } else {
      setIframeKey((k) => k + 1);
    }
  }, [webappMode, proxyAppUrl]);

  const handleBack = useCallback(() => {
    navigate(`/space/${id}`);
  }, [id, navigate]);

  const handleOpenInBrowser = useCallback(async () => {
    if (app) {
      const url = buildAppUrl(app);
      try {
        await open(url);
      } catch {
        window.open(url, "_blank");
      }
    }
  }, [app]);

  if (!id || !space) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)]">
        {t("common.spaceNotFound")}
      </div>
    );
  }

  if (loading || !app) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)]">
        {loading ? t("common.loading") : t("common.appNotFound")}
      </div>
    );
  }

  return (
    <div className="flex flex-col flex-1">
      {/* 顶部操作栏 */}
      <div className="h-12 flex items-center gap-3 px-4 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        <Button onClick={handleBack} variant="ghost" size="2" title={t("common.back")}>
          <ArrowLeft size={18} />
        </Button>
        <Button onClick={handleRefresh} variant="ghost" size="2" title={t("common.refresh")}>
          <RefreshCw size={18} />
        </Button>
        <Flex align="center" gap="2" className="flex-1 min-w-0">
          <div className="w-2 h-2 rounded-full bg-[var(--color-primary)] shrink-0" />
          <Text size="1" className="truncate font-mono text-[var(--color-text-secondary)]">
            {appUrl}
          </Text>
        </Flex>
        <Button onClick={handleOpenInBrowser} variant="ghost" size="2" title={t("common.openInBrowser")}>
          <ExternalLink size={16} />
        </Button>
      </div>

      {/* 内容主体 */}
      <div className="flex-1 bg-white relative">
        {webappMode === "webview" ? (
          webviewReady ? (
            <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-bg)]">
              <Flex direction="column" align="center" gap="3">
                <Monitor size={48} className="text-[var(--color-primary)]" />
                <Text size="2" className="text-[var(--color-text-secondary)]">
                  {t("common.webviewOpened")}
                </Text>
                <Button onClick={handleRefresh} variant="outline" size="2">
                  {t("common.refreshWindow")}
                </Button>
              </Flex>
            </div>
          ) : (
            <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-bg)]">
              <Loader2 size={32} className="animate-spin text-[var(--color-primary)]" />
            </div>
          )
        ) : proxyAppUrl && !loadError ? (
          <ProxyFrame
            key={iframeKey}
            proxyUrl={proxyAppUrl}
            name={app.name}
            onOpenBrowser={handleOpenInBrowser}
            onBack={handleBack}
            onError={() => setLoadError(true)}
          />
        ) : loadError ? (
          <ProxyErrorFallback onOpenBrowser={handleOpenInBrowser} onBack={handleBack} />
        ) : (
          <div className="absolute inset-0 flex items-center justify-center text-[var(--color-text-secondary)]">
            {t("common.invalidUrl")}
          </div>
        )}
      </div>
    </div>
  );
}
