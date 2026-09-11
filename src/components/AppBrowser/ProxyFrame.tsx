/* eslint-disable react-refresh/only-export-components */
import { useEffect, useLayoutEffect, useRef, useState, type TouchEvent, forwardRef, useImperativeHandle } from "react";
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

/** 缩放比例夹紧 [0.2, 2.0] */
const clampZoom = (v: number) => Math.max(0.2, Math.min(2.0, v));

/** 双指欧氏距离（ArrayLike 兼容 React.TouchList） */
const touchDist = (touches: ArrayLike<{ clientX: number; clientY: number }>) => {
  const a = touches[0];
  const b = touches[1];
  return Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY);
};

/** 双指中心点（pinch 缩放 + pan 平移同时） */
const touchCenter = (touches: ArrayLike<{ clientX: number; clientY: number }>) => ({
  x: (touches[0].clientX + touches[1].clientX) / 2,
  y: (touches[0].clientY + touches[1].clientY) / 2,
});

/** 限制 offset 使 viewport 不完全飞出容器（可 pan 到边缘，居中时固定） */
const clampOffset = (x: number, y: number, s: number, vw: number, vh: number, cw: number, ch: number) => {
  const sw = vw * s;
  const sh = vh * s;
  const minX = Math.min(cw - sw, 0);
  const maxX = Math.max(cw - sw, 0);
  const minY = Math.min(ch - sh, 0);
  const maxY = Math.max(ch - sh, 0);
  return { x: Math.max(minX, Math.min(maxX, x)), y: Math.max(minY, Math.min(maxY, y)) };
};

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
  const [zoomScale, setZoomScale] = useState(0.5);
  const [zoomOffset, setZoomOffset] = useState({ x: 0, y: 0 });
  const touchRef = useRef<{ mode: "none" | "drag" | "pinch"; startDist: number; startScale: number; startOffset: { x: number; y: number }; startTouch: { x: number; y: number }; startCenter: { x: number; y: number } }>({ mode: "none", startDist: 0, startScale: 0.5, startOffset: { x: 0, y: 0 }, startTouch: { x: 0, y: 0 }, startCenter: { x: 0, y: 0 } });
  // latest zoom state ref，使 message handler 不依赖 zoomScale/zoomOffset deps（避免高频重绑）
  const zoomStateRef = useRef({ scale: zoomScale, offset: zoomOffset });
  useEffect(() => { zoomStateRef.current = { scale: zoomScale, offset: zoomOffset }; }, [zoomScale, zoomOffset]);

  // 工具栏按钮命令（zoomIn/zoomOut/resetZoom）
  useImperativeHandle(ref, () => ({
    zoomIn: () => setZoomScale((s) => clampZoom(s + 0.1)),
    zoomOut: () => setZoomScale((s) => clampZoom(s - 0.1)),
    resetZoom: () => { setZoomScale(0.5); setZoomOffset({ x: 0, y: 0 }); },
  }), []);

  const onTouchStart = (e: TouchEvent<HTMLDivElement>) => {
    if (!enableZoom) return;
    const ts = e.touches;
    const zs = zoomStateRef.current;
    if (ts.length === 1) {
      touchRef.current = { mode: "drag", startDist: 0, startScale: zs.scale, startOffset: { ...zs.offset }, startTouch: { x: ts[0].clientX, y: ts[0].clientY }, startCenter: { x: 0, y: 0 } };
    } else if (ts.length >= 2) {
      touchRef.current = { mode: "pinch", startDist: touchDist(ts), startScale: zs.scale, startOffset: { ...zs.offset }, startTouch: { x: 0, y: 0 }, startCenter: touchCenter(ts) };
    }
  };
  const onTouchMove = (e: TouchEvent<HTMLDivElement>) => {
    if (!enableZoom) return;
    const st = touchRef.current;
    if (st.mode === "none") return;
    e.preventDefault();
    const ts = e.touches;
    if (st.mode === "drag" && ts.length >= 1) {
      const dx = ts[0].clientX - st.startTouch.x;
      const dy = ts[0].clientY - st.startTouch.y;
      setZoomOffset(clampOffset(st.startOffset.x + dx, st.startOffset.y + dy, st.startScale, viewport.w, viewport.h, cw, ch));
    } else if (st.mode === "pinch" && ts.length >= 2) {
      const ratio = st.startDist > 0 ? touchDist(ts) / st.startDist : 1;
      const newScale = clampZoom(st.startScale * ratio);
      setZoomScale(newScale);
      const c = touchCenter(ts);
      const dx = c.x - st.startCenter.x;
      const dy = c.y - st.startCenter.y;
      setZoomOffset(clampOffset(st.startOffset.x + dx, st.startOffset.y + dy, newScale, viewport.w, viewport.h, cw, ch));
    }
  };
  const onTouchEnd = (e: TouchEvent<HTMLDivElement>) => {
    if (!enableZoom) return;
    const st = touchRef.current;
    const zs = zoomStateRef.current;
    if (e.touches.length === 0) {
      touchRef.current = { ...st, mode: "none" };
    } else if (e.touches.length === 1 && st.mode === "pinch") {
      touchRef.current = { mode: "drag", startDist: 0, startScale: zs.scale, startOffset: { ...zs.offset }, startTouch: { x: e.touches[0].clientX, y: e.touches[0].clientY }, startCenter: { x: 0, y: 0 } };
    }
  };

  // 桌面端 Ctrl+滚轮缩放（wheel 需 non-passive 才能 preventDefault）
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const handler = (e: WheelEvent) => {
      if (!enableZoom || !e.ctrlKey) return;
      e.preventDefault();
      const delta = -e.deltaY * 0.0015;
      setZoomScale((s) => clampZoom(s * (1 + delta)));
    };
    el.addEventListener("wheel", handler, { passive: false });
    return () => el.removeEventListener("wheel", handler);
  }, [enableZoom, containerRef]);

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

  // 监听注入脚本上报：__ht_nav 导航状态 + __ht_touch iframe 内两指手势（pinch+pan）
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
      if (d.__ht_touch && enableZoom) {
        const td = d.__ht_touch;
        const st = touchRef.current;
        const ts = td.touches as { clientX: number; clientY: number }[];
        const zs = zoomStateRef.current;
        if (td.type === "touchstart" && ts.length >= 2) {
          touchRef.current = { mode: "pinch", startDist: touchDist(ts), startScale: zs.scale, startOffset: { ...zs.offset }, startTouch: { x: 0, y: 0 }, startCenter: touchCenter(ts) };
        } else if (td.type === "touchmove" && ts.length >= 2 && st.mode === "pinch") {
          const ratio = st.startDist > 0 ? touchDist(ts) / st.startDist : 1;
          const newScale = clampZoom(st.startScale * ratio);
          setZoomScale(newScale);
          const c = touchCenter(ts);
          const dx = c.x - st.startCenter.x;
          const dy = c.y - st.startCenter.y;
          setZoomOffset(clampOffset(st.startOffset.x + dx, st.startOffset.y + dy, newScale, viewport.w, viewport.h, cw, ch));
        } else if (td.type === "touchend") {
          if (st.mode === "pinch") touchRef.current = { ...st, mode: "none" };
        }
      }
    };
    window.addEventListener("message", handler);
    return () => window.removeEventListener("message", handler);
  }, [onNavState, proxyUrl, enableZoom, viewport, cw, ch]);

  // enableZoom（移动端+desktop）用手势 state；否则自适应（桌面端原生 desktop 100% 适应）
  const adaptiveScale = cw > 0 && ch > 0 ? Math.min(cw / viewport.w, ch / viewport.h) : 1;
  const scale = enableZoom ? zoomScale : adaptiveScale;
  const offsetX = enableZoom ? zoomOffset.x : (cw - viewport.w * adaptiveScale) / 2;
  const offsetY = enableZoom ? zoomOffset.y : (ch - viewport.h * adaptiveScale) / 2;

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
      style={{ touchAction: enableZoom ? "none" : undefined }}
      onTouchStart={enableZoom ? onTouchStart : undefined}
      onTouchMove={enableZoom ? onTouchMove : undefined}
      onTouchEnd={enableZoom ? onTouchEnd : undefined}
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
        style={{
          position: "absolute",
          left: 0,
          top: 0,
          width: viewport.w,
          height: viewport.h,
          transform: `translate(${offsetX}px, ${offsetY}px) scale(${scale})`,
          transformOrigin: "top left",
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