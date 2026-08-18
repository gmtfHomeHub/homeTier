import { useState } from "react";
import { useTranslation } from "react-i18next";
import { upgradeApp, isTauri } from "../../utils/api";
import { Badge, Button, Flex, Progress, Text } from "@radix-ui/themes";
import { Package, ArrowUpCircle } from "lucide-react";
import { useUpdateStore } from "../../stores/updateStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { toastSuccess, toastError } from "../../utils/toast";

const RELEASES_URL = "https://github.com/gmtfHomeHub/homeTier/releases";

export function AppVersionManager() {
  const { t } = useTranslation();
  const currentVersion = useUpdateStore((s) => s.currentVersion);
  const latestVersion = useUpdateStore((s) => s.latestVersion);
  const hasUpdate = useUpdateStore((s) => s.hasUpdate);
  const checked = useUpdateStore((s) => s.checked);
  const useProxy = useSettingsStore((s) => s.useProxy);
  const [upgrading, setUpgrading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<number | null>(null);

  const openReleases = () => {
    if (isTauri()) {
      import("@tauri-apps/plugin-shell").then(({ open }) => {
        open(RELEASES_URL).catch((e) => toastError(String(e)));
      });
    } else {
      window.open(RELEASES_URL, "_blank");
    }
  };

  const handleUpdate = async () => {
    setUpgrading(true);
    setDownloadProgress(0);
    try {
      const outcome = await upgradeApp(useProxy, (pct) => setDownloadProgress(pct));
      if (outcome.action === "installed") {
        setDownloadProgress(null);
        toastSuccess(t("settings.updateReadyRestart"));
      } else {
        setDownloadProgress(null);
        openReleases();
      }
    } catch (e) {
      setDownloadProgress(null);
      toastError(String(e));
    } finally {
      setUpgrading(false);
    }
  };

  return (
    <section className="border border-[var(--color-border)] rounded-lg space-y-3">
      <Flex align="center" gap="2">
        <Package size={16} className="text-[var(--color-primary)]" />
        <Text size="2" weight="bold">{t("settings.appEngine")}</Text>
        {hasUpdate && (
          <Badge color="cyan" variant="soft">New</Badge>
        )}
      </Flex>

      <Flex align="center" justify="between">
        <div className="grid grid-cols-2 gap-2 text-xs">
          <Text className="text-[var(--color-text-secondary)]">{t("settings.currentVersion")}</Text>
          <Text>{currentVersion ?? "-"}</Text>
          {hasUpdate && latestVersion && (
            <>
              <Text className="text-[var(--color-text-secondary)]">{t("settings.latestVersion")}</Text>
              <Text className="text-[var(--color-primary)]">{latestVersion}</Text>
            </>
          )}
        </div>

        <Flex gap="2" wrap="wrap">
          {hasUpdate ? (
            <Button
              onClick={handleUpdate}
              disabled={upgrading}
              variant="ghost"
              size="1"
              loading={upgrading}
            >
              <ArrowUpCircle size={14} />
              {t("settings.update")}
            </Button>
          ) : (
            checked && (
              <Text size="1" className="text-[var(--color-text-secondary)]">
                {t("settings.upToDate")}
              </Text>
            )
          )}
        </Flex>
      </Flex>

      {downloadProgress !== null && (
        <div className="space-y-1">
          <Progress value={Math.round(downloadProgress)} size="1" />
          <Text size="1" className="text-[var(--color-text-secondary)]">
            {t("settings.downloadProgress", { pct: Math.round(downloadProgress) })}
          </Text>
        </div>
      )}
    </section>
  );
}
