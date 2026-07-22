import { useRef } from "react";
import { ShieldAlert } from "lucide-react";
import { Button, Flex, Text, Card } from "@radix-ui/themes";

interface ProxyFrameProps {
  proxyUrl: string;
  name: string;
  onOpenBrowser: () => void;
  onBack: () => void;
  onError?: () => void;
}

export function ProxyFrame({ proxyUrl, name, onOpenBrowser, onBack, onError }: ProxyFrameProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  return (
    <iframe
      ref={iframeRef}
      src={proxyUrl}
      className="w-full h-full border-none"
      title={name}
      sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals"
      onError={onError}
    />
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
  return (
    <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-bg)]">
      <Card className="max-w-md p-6 text-center">
        <ShieldAlert size={48} className="mx-auto mb-4 text-[var(--color-text-secondary)]" />
        <Text size="3" weight="bold" className="block mb-2">
          无法在内部加载
        </Text>
        <Text size="2" className="text-[var(--color-text-secondary)] block mb-4">
          该网站无法通过代理加载。请点击下方按钮在系统浏览器中打开。
        </Text>
        <Flex gap="3" justify="center">
          <Button onClick={onOpenBrowser} variant="solid" color="blue" size="2">
            在浏览器中打开
          </Button>
          <Button onClick={onBack} variant="outline" size="2">
            返回
          </Button>
        </Flex>
      </Card>
    </div>
  );
}
