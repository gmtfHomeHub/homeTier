import { useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ShieldAlert } from "lucide-react";
import { Button, Flex, Text, Card } from "@radix-ui/themes";
import { DEVICE_VIEWPORTS, type DeviceMode } from "../../utils/device";

interface ProxyFrameProps {
  proxyUrl: string;
  name: string;
  deviceMode: DeviceMode;
  onOpenBrowser: () => void;
  onBack: () => void;
  onError?: () => void;
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

export function ProxyFrame({ proxyUrl, name, deviceMode, onOpenBrowser, onBack, onError }: ProxyFrameProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const { ref: containerRef, width: cw, height: ch } = useContainerSize<HTMLDivElement>();

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
          src={proxyUrl}
          className="border-none"
          style={{ width: viewport.w, height: viewport.h }}
          title={name}
          sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals"
          onError={onError}
        />
      </div>
    </div>
  );
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
