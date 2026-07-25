import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { getTunStatus, authorizeTun, refreshTunStatus } from "../../utils/api";
import type { TunStatus, AuthResult } from "../../types";
import { Button, Text, Flex } from "@radix-ui/themes";
import { Shield, ShieldOff, RotateCw } from "lucide-react";

export function TunAuthPanel() {
  const { t } = useTranslation();
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
        <Text size="2" weight="bold">{t("settings.tunStatus")}</Text>
        <Flex gap="1" ml="auto">
          <Button variant="ghost" size="1" onClick={load} title={t("settings.refresh")}>
            <RotateCw size={14} />
          </Button>
        </Flex>
      </Flex>

      <div className="grid grid-cols-2 gap-2 text-xs">
        <Text className="text-[var(--color-text-secondary)]">{t("settings.platform")}</Text>
        <Text>{platformLabels[status.platform] ?? status.platform}</Text>
        <Text className="text-[var(--color-text-secondary)]">{t("settings.tunAvailable")}</Text>
        <Text className={status.tun_available ? "text-green-500" : "text-red-500"}>
          {status.tun_available ? `${t("settings.yes")} ✓` : `${t("settings.no")} ✗`}
        </Text>
        <Text className="text-[var(--color-text-secondary)]">{t("settings.elevationStatus")}</Text>
        <Text>{status.elevated ? t("settings.elevated") : t("settings.notElevated")}</Text>
      </div>

      {!status.tun_available && (
        <div className="space-y-2">
          <Text size="1" className="text-[var(--color-text-secondary)]">
            {t("settings.tunNotAvailable")}
            {status.platform === "linux" && ` ${t("settings.tunHelpLinux")}`}
            {status.platform === "windows" && ` ${t("settings.tunHelpWindows")}`}
            {status.platform === "macos" && (
              <>
                {t("settings.tunHelpMacOS")}{" "}
                <code className="text-xs bg-[var(--color-bg-secondary)] px-1 rounded">
                  sudo hometier --daemon
                </code>
              </>
            )}
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
              {authorizing ? t("settings.authorizing") : `🔒 ${t("settings.authorize")}`}
            </Button>
            {lastResult && (
              <Text size="1" className={lastResult.success ? "text-green-500" : "text-red-500"}>
                {lastResult.message}
                {lastResult.success && lastResult.needs_restart && t("settings.restartToApply")}
              </Text>
            )}
          </Flex>
        </div>
      )}

      {status.tun_available && (
        <Text size="1" className="text-green-500">
          {t("settings.tunReady")}
        </Text>
      )}
    </section>
  );
}
