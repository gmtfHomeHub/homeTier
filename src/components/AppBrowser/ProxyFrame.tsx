import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ShieldAlert, Loader2 } from "lucide-react";
import { Button, Flex, Text, Card } from "@radix-ui/themes";
import { listen } from "@tauri-apps/api/event";
import { DEVICE_VIEWPORTS, type DeviceMode } from "../../utils/device";
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

export function ProxyFrame({ tabKey, proxyUrl, name, deviceMode, onOpenBrowser, onBack, onError, onNavState }: ProxyFrameProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const { ref: containerRef, width: cw, height: ch } = useContainerSize<HTMLDivElement>();
  const { t } = useTranslation();
  const [loading, setLoading] = useState(true);
  const [stage, setStage] = useState("connecting");
  const proxyKey = parseProxyKey(proxyUrl);

  // 监听后端代理转发进度，按 key 匹配更新阶段文案
  useEffect(() => {
    if (!proxyKey) return;
    const un = listen<{ key: string; stage: string }>("proxy:load-progress", (e) => {
      if (e.payload.key !== proxyKey) return;
      setStage(e.payload.stage);
      if (e.payload.stage === "error") setLoading(false);
    });
    return () => { un.then((fn) => fn()); };
  }, [proxyKey]);

  // 超时兑底：每个阶段 10s 无新事件则提示慢；stage 变化重置计时
  useEffect(() => {
    if (!loading) return;
    const timer = setTimeout(() => setStage("slow"), 10000);
    return () => clearTimeout(timer);
  }, [loading, stage]);

  // 监听注入脚本的导航状态上报（__ht_nav），桥接给工具栏
  useLayoutEffect(() => {
    const handler = (e: MessageEvent) => {
      if (e.source !== iframeRef.current?.contentWindow) return;
      const d = e.data;
      if (!d || !d.__ht_nav) return;
      onNavState?.({
        canBack: d.idx > 0,
        canFwd: d.idx < d.len - 1,
        url: typeof d.url === "string" ? d.url : proxyUrl,
      });
    };
    window.addEventListener("message", handler);
    return () => window.removeEventListener("message", handler);
  }, [onNavState, proxyUrl]);

  const viewport = DEVICE_VIEWPORTS[deviceMode];
  const scale = cw > 0 && ch > 0 ? Math.min(cw / viewport.w, ch / viewport.h) : 1;
  const offsetX = (cw - viewport.w * scale) / 2;
  const offsetY = (ch - viewport.h * scale) / 2;

  const STAGE_TEXT: Record<string, string> = {
    connecting: t("common.proxyLoadingConnecting"),
    fetching: t("common.proxyLoadingFetching"),
    processing: t("common.proxyLoadingProcessing"),
    ready: t("common.proxyLoadingReady"),
    error: t("common.proxyLoadError"),
    slow: t("common.proxyLoadingSlow"),
  };

  return (
    <div ref={containerRef} className="absolute inset-0 overflow-hidden bg-white">
      {loading && (
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
          left: offsetX,
          top: offsetY,
          width: viewport.w,
          height: viewport.h,
          transform: `scale(${scale})`,
          transformOrigin: "top left",
        }}
      >
        <iframe
          ref={iframeRef}
          id={`ht-frame-${tabKey}`}
          src={proxyUrl}
          className="border-none"
          style={{ width: viewport.w, height: viewport.h }}
          title={name}
          sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals allow-pointer-lock allow-popups-to-escape-sandbox allow-top-navigation"
          allow="fullscreen; camera; microphone; display-capture; focus"
          onLoad={() => setLoading(false)}
          onError={onError}
        />
      </div>
    </div>
  );
}

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
