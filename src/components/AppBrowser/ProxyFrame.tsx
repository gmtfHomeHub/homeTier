import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ShieldAlert } from "lucide-react";
import { Button, Flex, Text, Card } from "@radix-ui/themes";
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

export function ProxyFrame({ tabKey, proxyUrl, name, deviceMode, onOpenBrowser, onBack, onError, onNavState }: ProxyFrameProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const { ref: containerRef, width: cw, height: ch } = useContainerSize<HTMLDivElement>();

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

  return (
    <div ref={containerRef} className="absolute inset-0 overflow-hidden bg-white">
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
          allow="fullscreen; camera; microphone; display-capture"
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

function buildProxyUrl(originalUrl: string): string {
  try {
    const u = new URL(originalUrl);
    const hostPort = u.port ? `${u.hostname}:${u.port}` : u.hostname;
    return `hometierproxy://${hostPort}${u.pathname}${u.search}${u.hash}`;
  } catch {
    return originalUrl;
  }
}

export const BROWSER_ENGINE_KEY = "APP_BROWSER_ENGINE";

/** 解析应用 URL 到代理内 URL：优先 localHttp（真实 http origin），失败回退 hometierproxy 自定义协议 */
export async function resolveProxyUrl(
  originalUrl: string
): Promise<{ url: string; engine: "local-http" | "hometierproxy" }> {
  try {
    const cfg = await api.getAppConfig();
    const engine = cfg?.[BROWSER_ENGINE_KEY] === "hometierproxy" ? "hometierproxy" : "local-http";
    if (engine === "hometierproxy") {
      return { url: buildProxyUrl(originalUrl), engine };
    }
    const proxyUrl = await api.getProxyUrl();
    const proxy = new URL(proxyUrl);
    const key = await api.registerProxyKey(originalUrl);
    const u = new URL(originalUrl);
    const path = u.pathname === "/" ? "" : u.pathname;
    return {
      url: `http://127.0.0.1:${proxy.port}/__proxy__${key}${path}${u.search}${u.hash}`,
      engine,
    };
  } catch {
    return { url: buildProxyUrl(originalUrl), engine: "hometierproxy" };
  }
}

export { buildProxyUrl };

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
