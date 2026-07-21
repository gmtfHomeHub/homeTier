import { useState, useRef, useCallback, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { ArrowLeft, RefreshCw, ExternalLink, ShieldAlert, Loader2, Monitor } from "lucide-react";
import { Button, Flex, Text, Card } from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { open } from "@tauri-apps/plugin-shell";
import * as api from "../../utils/api";
import { buildAppUrl } from "../../types";
import type { SpaceApp } from "../../types";

export function AppBrowserView() {
  const { id, appId } = useParams<{ id: string; appId: string }>();
  const navigate = useNavigate();
  const { spaces } = useSpaceStore();
  const [app, setApp] = useState<SpaceApp | null>(null);
  const [loading, setLoading] = useState(true);
  const [iframeKey, setIframeKey] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [proxyUrl, setProxyUrl] = useState<string | null>(null);
  const [proxyKey, setProxyKey] = useState<string | null>(null);
  const [proxyLoading, setProxyLoading] = useState(true);
  const [webappMode, setWebappMode] = useState<string>("iframe");
  const [webviewReady, setWebviewReady] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const space = spaces.find((s) => s.id === id);

  // 加载应用信息
  useEffect(() => {
    if (id && appId) {
      api.listApps(id).then((apps) => {
        const found = apps.find((a) => a.id === appId);
        setApp(found || null);
        setLoading(false);
      });
    }
  }, [id, appId]);

  // 获取代理地址并注册 key
  useEffect(() => {
    api.getProxyUrl()
      .then((url) => {
        console.log("[AppBrowser] proxyUrl:", url);
        setProxyUrl(url);
        setProxyLoading(false);
      })
      .catch((e) => {
        console.error("[AppBrowser] getProxyUrl failed:", e);
        setProxyLoading(false);
      });
  }, []);

  // 注册 proxy key
  useEffect(() => {
    const appUrl = app ? buildAppUrl(app) : null;
    if (appUrl) {
      api.registerProxyKey(appUrl)
        .then((key) => {
          setProxyKey(key);
          api.setProxySource(appUrl);
        })
        .catch((e) => {
          console.error("[AppBrowser] registerProxyKey failed:", e);
        });
    }
  }, [app]);

  // 读取 WebView 模式
  useEffect(() => {
    api.getWebappMode().then(setWebappMode);
  }, []);

  // webview 模式下自动打开
  useEffect(() => {
    if (webappMode !== "webview" || !app || !proxyUrl || !proxyKey) return;
    const appUrl = buildAppUrl(app);
    const appPath = appUrl.replace(/^https?:\/\/[^\/]+/, "");
    const proxyAppUrl = `${proxyUrl}/__proxy__${proxyKey}${appPath || "/"}`;
    api.openAppView(proxyAppUrl, 0, 56, window.innerWidth, window.innerHeight - 56)
      .then(() => setWebviewReady(true))
      .catch(console.error);
  }, [webappMode, app, proxyUrl, proxyKey]);

  // webview 模式下监听窗口 resize
  useEffect(() => {
    if (webappMode !== "webview") return;
    const handler = () => {
      api.resizeAppView(0, 56, window.innerWidth, window.innerHeight - 56)
        .catch(console.error);
    };
    window.addEventListener("resize", handler);
    return () => window.removeEventListener("resize", handler);
  }, [webappMode]);

  // 离开时关闭 webview
  useEffect(() => {
    return () => {
      api.closeAppView().catch(console.error);
    };
  }, []);

  const handleRefresh = useCallback(() => {
    setLoadError(false);
    if (webappMode === "webview") {
      const appUrl = app ? buildAppUrl(app) : null;
      const appPath = appUrl ? appUrl.replace(/^https?:\/\/[^\/]+/, "") : "/";
      const proxyAppUrl = proxyUrl && proxyKey
        ? `${proxyUrl}/__proxy__${proxyKey}${appPath || "/"}`
        : null;
      if (proxyAppUrl) {
        api.closeAppView().then(() => {
          setWebviewReady(false);
          return api.openAppView(proxyAppUrl, 0, 56, window.innerWidth, window.innerHeight - 56);
        }).then(() => setWebviewReady(true)).catch(console.error);
      }
    } else {
      setIframeKey((k) => k + 1);
    }
  }, [webappMode, app, proxyUrl, proxyKey]);

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
        空间不存在
      </div>
    );
  }

  if (loading || !app) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)]">
        {loading ? "加载中..." : "应用不存在"}
      </div>
    );
  }

  const appUrl = buildAppUrl(app);
  const appPath = appUrl.replace(/^https?:\/\/[^\/]+/, "");
  const proxyAppUrl = proxyUrl && proxyKey
    ? `${proxyUrl}/__proxy__${proxyKey}${appPath || "/"}`
    : null;

  return (
    <div className="flex flex-col flex-1">
      {/* 顶部操作栏 */}
      <div className="h-12 flex items-center gap-3 px-4 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        <Button onClick={handleBack} variant="ghost" size="2" title="返回">
          <ArrowLeft size={18} />
        </Button>
        <Button onClick={handleRefresh} variant="ghost" size="2" title="刷新">
          <RefreshCw size={18} />
        </Button>
        <Flex align="center" gap="2" className="flex-1 min-w-0">
          <div className="w-2 h-2 rounded-full bg-[var(--color-primary)] shrink-0" />
          <Text size="1" className="truncate font-mono text-[var(--color-text-secondary)]">
            {appUrl}
          </Text>
        </Flex>
        <Button onClick={handleOpenInBrowser} variant="ghost" size="2" title="在浏览器中打开">
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
                  应用已在独立 WebView 窗口中打开
                </Text>
                <Button onClick={handleRefresh} variant="outline" size="2">
                  刷新窗口
                </Button>
              </Flex>
            </div>
          ) : (
            <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-bg)]">
              <Loader2 size={32} className="animate-spin text-[var(--color-primary)]" />
            </div>
          )
        ) : proxyLoading ? (
          <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-bg)]">
            <Loader2 size={32} className="animate-spin text-[var(--color-primary)]" />
          </div>
        ) : !proxyUrl && !proxyLoading ? (
          <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-bg)]">
            <Card className="max-w-md p-6 text-center">
              <ShieldAlert size={48} className="mx-auto mb-4 text-[var(--color-text-secondary)]" />
              <Text size="3" weight="bold" className="block mb-2">
                代理服务不可用
              </Text>
              <Text size="2" className="text-[var(--color-text-secondary)] block mb-4">
                代理服务未启动或已停止，请点击下方按钮在系统浏览器中打开。
              </Text>
              <Flex gap="3" justify="center">
                <Button onClick={handleOpenInBrowser} variant="solid" color="blue" size="2">
                  <ExternalLink size={16} /> 在浏览器中打开
                </Button>
                <Button onClick={handleBack} variant="outline" size="2">
                  返回
                </Button>
              </Flex>
            </Card>
          </div>
        ) : loadError ? (
          <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-bg)]">
            <Card className="max-w-md p-6 text-center">
              <ShieldAlert size={48} className="mx-auto mb-4 text-[var(--color-text-secondary)]" />
              <Text size="3" weight="bold" className="block mb-2">
                无法在内部加载
              </Text>
              <Text size="2" className="text-[var(--color-text-secondary)] block mb-4">
                该网站无法通过代理加载。
                请点击下方按钮在系统浏览器中打开。
              </Text>
              <Flex gap="3" justify="center">
                <Button onClick={handleOpenInBrowser} variant="solid" color="blue" size="2">
                  <ExternalLink size={16} /> 在浏览器中打开
                </Button>
                <Button onClick={handleBack} variant="outline" size="2">
                  返回
                </Button>
              </Flex>
            </Card>
          </div>
        ) : (
          <iframe
            key={iframeKey}
            ref={iframeRef}
            src={proxyAppUrl || appUrl}
            className="w-full h-full border-none"
            title={app.name}
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals"
            onError={() => setLoadError(true)}
          />
        )}
      </div>
    </div>
  );
}