import { useState, useEffect, useCallback } from "react";
import { getTunStatus, authorizeTun, refreshTunStatus } from "../../utils/api";
import type { TunStatus, AuthResult } from "../../types";
import { Button, Text, Flex } from "@radix-ui/themes";
import { Shield, ShieldOff, RotateCw } from "lucide-react";

export function TunAuthPanel() {
  const [status, setStatus] = useState<TunStatus | null>(null);
  const [authorizing, setAuthorizing] = useState(false);
  const [lastResult, setLastResult] = useState<AuthResult | null>(null);

  const load = useCallback(async () => {
    const s = await getTunStatus();
    setStatus(s);
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleAuthorize = async () => {
    setAuthorizing(true);
    setLastResult(null);
    try {
      const r = await authorizeTun();
      setLastResult(r);
      // 刷新状态
      const s = await refreshTunStatus();
      setStatus(s);
    } catch (e) {
      setLastResult({ success: false, message: String(e), needs_restart: false });
    } finally {
      setAuthorizing(false);
    }
  };

  if (!status) return null;

  const platformLabels: Record<string, string> = {
    linux: "Linux",
    windows: "Windows",
    macos: "macOS",
    android: "Android",
    ios: "iOS",
  };

  return (
    <section className="border border-[var(--color-border)] rounded-lg p-4 space-y-3">
      <Flex align="center" gap="2">
        {status.tun_available
          ? <Shield size={16} className="text-green-500" />
          : <ShieldOff size={16} className="text-red-500" />
        }
        <Text size="2" weight="bold">虚拟网卡 (TUN) 状态</Text>
        <Flex gap="1" ml="auto">
          <Button variant="ghost" size="1" onClick={load} title="刷新">
            <RotateCw size={14} />
          </Button>
        </Flex>
      </Flex>

      <div className="grid grid-cols-2 gap-2 text-xs">
        <Text className="text-[var(--color-text-secondary)]">平台</Text>
        <Text>{platformLabels[status.platform] ?? status.platform}</Text>
        <Text className="text-[var(--color-text-secondary)]">TUN 可用</Text>
        <Text className={status.tun_available ? "text-green-500" : "text-red-500"}>
          {status.tun_available ? "是 ✓" : "否 ✗"}
        </Text>
        <Text className="text-[var(--color-text-secondary)]">提权状态</Text>
        <Text>{status.elevated ? "已提权" : "未提权"}</Text>
      </div>

      {!status.tun_available && (
        <div className="space-y-2">
          <Text size="1" className="text-[var(--color-text-secondary)]">
            虚拟网卡需要系统级权限才能创建。
            {status.platform === "linux" && " 将弹出系统授权对话框以设置 cap_net_admin 能力。"}
            {status.platform === "windows" && " 将弹出 UAC 对话框以管理员身份运行。"}
            {status.platform === "macos" && " 将请求管理员权限。"}
          </Text>
          <Flex gap="2" align="center">
            <Button
              onClick={handleAuthorize}
              disabled={authorizing}
              variant="solid"
              color="blue"
              size="2"
              loading={authorizing}
            >
              {authorizing ? "授权中..." : "🔒 授权"}
            </Button>
            {lastResult && (
              <Text size="1" className={lastResult.success ? "text-green-500" : "text-red-500"}>
                {lastResult.message}
                {lastResult.success && lastResult.needs_restart && "（重启后生效）"}
              </Text>
            )}
          </Flex>
        </div>
      )}

      {status.tun_available && (
        <Text size="1" className="text-green-500">
          TUN 设备已就绪，可以创建虚拟网卡
        </Text>
      )}
    </section>
  );
}
