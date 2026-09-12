/* eslint-disable react-refresh/only-export-components */
import { useEffect, useLayoutEffect, useRef, useState, forwardRef, useImperativeHandle } from "react";
import { useTranslation } from "react-i18next";
import { ShieldAlert, Loader2 } from "lucide-react";
import { Button, Flex, Text, Card } from "@radix-ui/themes";
import { listen } from "@tauri-apps/api/event";
import { DEVICE_VIEWPORTS, useIsMobilePlatform, type DeviceMode } from "../../utils/device";
import * as api from "../../utils/api";

export interface FrameNavState {
  canBack: boolean;
  canFwd: boolean;
  url: string;
}

interface ProxyFrameProps {
  tabKey: string;
  proxyUrl: string;
  name: string;
  deviceMode: DeviceMode;
  refreshNonce: number;
  onOpenBrowser: () => void;
  onBack: () => void;
  onError?: () => void;
  onNavState?: (state: FrameNavState) => void;
}

function useContainerSize<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => setSize({ width: el.clientWidth, height: el.clientHeight });
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return { ref, ...size };
}

/** 从代理 URL 提取 proxy key，用于关联后端加载进度事件 */
function parseProxyKey(proxyUrl: string): string {
  const m = proxyUrl.match(/\/__proxy__([^/?]+)/);
  return m?.[1] ?? "";
}


export interface ProxyFrameHandle {
  zoomIn: () => void;
  zoomOut: () => void;
  resetZoom: () => void;
}

export const ProxyFrame = forwardRef<ProxyFrameHandle, ProxyFrameProps>(function ProxyFrame({ tabKey, proxyUrl, name, deviceMode, refreshNonce, onError, onNavState }, ref) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const { ref: containerRef, width: cw, height: ch } = useContainerSize<HTMLDivElement>();
  const { t } = useTranslation();

  // 缩放/拖拽：仅移动端平台 + desktop 模式启用 50% + 手势；
  // 桌面端原生 desktop 自适应 100%，mobile 模式自适应
  const mobilePlatform = useIsMobilePlatform();
  const isDesktop = deviceMode === "desktop";
  const enableZoom = mobilePlatform && isDesktop;
  const viewport = DEVICE_VIEWPORTS[deviceMode];
  // 方案 C：iframe 内自管 transform（TOUCH_BRIDGE_JS），parent 仅 postMessage 控制
  const frameWrapRef = useRef<HTMLDivElement>(null);

  // 工具栏按钮 → postMessage iframe __ht_zoom_cmd（iframe 内自管 zoom）
  useImperativeHandle(ref, () => ({
    zoomIn: () => iframeRef.current?.contentWindow?.postMessage({ __ht_zoom_cmd: "in" }, "*"),
    zoomOut: () => iframeRef.current?.contentWindow?.postMessage({ __ht_zoom_cmd: "out" }, "*"),
    resetZoom: () => iframeRef.current?.contentWindow?.postMessage({ __ht_zoom_cmd: "reset" }, "*"),
  }), []);


  const [loading, setLoading] = useState(true);
  const [stage, setStage] = useState("connecting");
  const loadedRef = useRef(false);
  const retriedRef = useRef(false);
  const loadSessionRef = useRef(0);
  const sessionStartRef = useRef(Date.now());
  const [retryNonce, setRetryNonce] = useState(0);
  const proxyKey = parseProxyKey(proxyUrl);

  // refreshNonce 变化 → 外层已通过 key 重建本组件（iframe 全新加载），
  // 这里只重置会话状态 + loading，并兜底强制解除遮罩防止永久白屏/黑屏
  const reloadTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  useEffect(() => {
    if (refreshNonce > 0) {
      loadSessionRef.current++;
      sessionStartRef.current = Date.now();
      loadedRef.current = false;
      retriedRef.current = false;
      setLoading(true);
      setStage("connecting");
      // 兜底：8s 内未触发 onLoad（iframe 白屏/跨域阻塞）时强制解除遮罩，避免永久白屏/黑屏
      reloadTimeoutRef.current = setTimeout(() => {
        if (!loadedRef.current) {
          setStage("slow");
          setLoading(false);
        }
      }, 8000);
    }
    return () => {
      if (reloadTimeoutRef.current) clearTimeout(reloadTimeoutRef.current);
    };
  }, [refreshNonce]);

  // 监听后端代理转发进度，按 key + session 匹配（隔离旧请求的残留事件）
  // 依赖 refreshNonce：当刷新计数器变化时重新绑定监听器，使 session 匹配新会话
  useEffect(() => {
    if (!proxyKey) return;
    const session = loadSessionRef.current;
    const un = listen<{ key: string; stage: string; error?: string }>("proxy:load-progress", (e) => {
      if (e.payload.key !== proxyKey) return;
      if (loadSessionRef.current !== session) return;
      setStage(e.payload.stage);
      if (e.payload.stage === "error") {
        const isRetriable = (e.payload.error?.includes("connect_timeout") ?? false)
          || (e.payload.error?.includes("connect_failed") ?? false);
        if (!retriedRef.current && isRetriable) {
          retriedRef.current = true;
          // 不隐藏 loading，直接进入重试；重试逻辑会重置 loading 状态
          setTimeout(() => {
            // 不递增 loadSessionRef，使重试的进度事件仍被当前监听器接收
            sessionStartRef.current = Date.now();
            loadedRef.current = false;
            setLoading(true);
            setStage("connecting");
            // iframe key 递增强制重建重载（无跨源访问、无自赋值 lint 问题）
            setRetryNonce((n) => n + 1);
          }, 2000);
        } else {
          // 非可重试错误或已重试过：显示错误状态
          setLoading(false);
        }
      }
    });
    return () => { un.then((fn) => fn()); };
  }, [proxyKey, refreshNonce]);

  // 固定 10s 超时提示（不再随 stage 变化重置）
  useEffect(() => {
    if (!loading) return;
    const session = loadSessionRef.current;
    const timer = setTimeout(() => {
      if (loadSessionRef.current === session) setStage("slow");
    }, 10000);
    return () => clearTimeout(timer);
  }, [loading]);

  // 监听注入脚本上报：__ht_nav 导航状态（iframe 内手势已自管，不再转发 parent）
  useLayoutEffect(() => {
    const handler = (e: MessageEvent) => {
      if (e.source !== iframeRef.current?.contentWindow) return;
      const d = e.data;
      if (!d) return;
      if (d.__ht_nav) {
        onNavState?.({
          canBack: d.idx > 0,
          canFwd: d.idx < d.len - 1,
          url: typeof d.url === "string" ? d.url : proxyUrl,
        });
        return;
      }
    };
    window.addEventListener("message", handler);
    return () => window.removeEventListener("message", handler);
  }, [onNavState, proxyUrl]);

  // 方案 C：desktop 模式 iframe 内自管 transform（TOUCH_BRIDGE_JS），parent 仅 postMessage __ht_zoom_init
  // 通知激活+初始 scale+cw/ch；mobile 模式 parent 设 wrapper transform 自适应 + postMessage 不激活
  useLayoutEffect(() => {
    const w = iframeRef.current?.contentWindow;
    if (enableZoom) {
      w?.postMessage({ __ht_zoom_init: { active: true, scale: 0.5, cw, ch } }, "*");
    } else {
      const el = frameWrapRef.current;
      if (el) {
        const s = cw > 0 && ch > 0 ? Math.min(cw / viewport.w, ch / viewport.h) : 1;
        const ox = (cw - viewport.w * s) / 2;
        const oy = (ch - viewport.h * s) / 2;
        el.style.transform = `translate(${ox}px, ${oy}px) scale(${s})`;
      }
      w?.postMessage({ __ht_zoom_init: { active: false } }, "*");
    }
  }, [enableZoom, cw, ch, viewport.w, viewport.h]);

  const STAGE_TEXT: Record<string, string> = {
    connecting: t("common.proxyLoadingConnecting"),
    fetching: t("common.proxyLoadingFetching"),
    processing: t("common.proxyLoadingProcessing"),
    ready: t("common.proxyLoadingReady"),
    error: t("common.proxyLoadError"),
    slow: t("common.proxyLoadingSlow"),
  };

  return (
    <div
      ref={containerRef}
      className="absolute inset-0 overflow-hidden bg-white"
    >
      {loading && !loadedRef.current && (
        <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-white/85 backdrop-blur-sm">
          <Loader2 size={32} className="animate-spin text-[var(--color-primary)]" />
          <Text size="2" className="text-[var(--color-text-secondary)]">
            {STAGE_TEXT[stage] ?? t("common.proxyLoading")}
          </Text>
        </div>
      )}
      <div
        ref={frameWrapRef}
        style={{
          position: "absolute",
          left: 0,
          top: 0,
          width: viewport.w,
          height: viewport.h,
          transformOrigin: "top left",
          willChange: "transform",
        }}
      >
        <iframe
          key={retryNonce}
          ref={iframeRef}
          id={`ht-frame-${tabKey}`}
          src={proxyUrl}
          className="border-none"
          style={{ width: viewport.w, height: viewport.h }}
          title={name}
          sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals allow-pointer-lock allow-popups-to-escape-sandbox allow-top-navigation"
          allow="fullscreen; camera; microphone; display-capture; focus"
          onLoad={() => {
            if (!loadedRef.current) {
              loadedRef.current = true;
              const elapsed = Date.now() - sessionStartRef.current;
              const remaining = Math.max(0, 300 - elapsed);
              const session = loadSessionRef.current;
              if (remaining > 0) {
                setTimeout(() => {
                  if (loadSessionRef.current === session) setLoading(false);
                }, remaining);
              } else {
                setLoading(false);
              }
            }
            // 方案 C：iframe load 后通知激活/模式（contentWindow ready）
            iframeRef.current?.contentWindow?.postMessage({ __ht_zoom_init: { active: enableZoom, scale: 0.5, cw, ch } }, "*");
          }}
          onError={onError}
        />
      </div>
    </div>
  );
});

/** 向 iframe 内注入的导航桥发送命令 */
export function sendFrameNavCmd(tabKey: string, cmd: "back" | "forward" | "go", url?: string) {
  const el = document.getElementById(`ht-frame-${tabKey}`) as HTMLIFrameElement | null;
  el?.contentWindow?.postMessage({ __ht_nav_cmd: { cmd, url } }, "*");
}

/** 解析应用 URL 到本地 HTTP 代理 URL（local-http 引擎，唯一代理方案） */
export async function resolveProxyUrl(originalUrl: string): Promise<string> {
  const proxyUrl = await api.getProxyUrl();
  const proxy = new URL(proxyUrl);
  const key = await api.registerProxyKey(originalUrl);
  const u = new URL(originalUrl);
  const path = u.pathname === "/" ? "" : u.pathname;
  return `http://127.0.0.1:${proxy.port}/__proxy__${key}${path}${u.search}${u.hash}`;
}

interface ErrorFallbackProps {
  onOpenBrowser: () => void;
  onBack: () => void;
}

export function ProxyErrorFallback({ onOpenBrowser, onBack }: ErrorFallbackProps) {
  const { t } = useTranslation();
  return (
    <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-bg)]">
      <Card className="max-w-md p-6 text-center">
        <ShieldAlert size={48} className="mx-auto mb-4 text-[var(--color-text-secondary)]" />
        <Text size="3" weight="bold" className="block mb-2">
          {t("common.proxyLoadError")}
        </Text>
        <Text size="2" className="text-[var(--color-text-secondary)] block mb-4">
          {t("common.proxyLoadErrorDescription")}
        </Text>
        <Flex gap="3" justify="center">
          <Button onClick={onOpenBrowser} variant="solid" color="blue" size="2">
            {t("common.openInBrowser")}
          </Button>
          <Button onClick={onBack} variant="outline" size="2">
            {t("common.back")}
          </Button>
        </Flex>
      </Card>
    </div>
  );
}