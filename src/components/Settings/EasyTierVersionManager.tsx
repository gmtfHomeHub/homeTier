import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getEasyTierVersion, checkEasyTierUpdate, upgradeEasyTierWithProgress, buildEasyTierFromSource } from "../../utils/api";
import { Button, Text, Flex, Select, Switch, Progress } from "@radix-ui/themes";
import { Cpu, RefreshCw, ArrowUpCircle, Hammer } from "lucide-react";
import { useSettingsStore } from "../../stores/settingsStore";

export function EasyTierVersionManager() {
  const { t } = useTranslation();
  const useProxy = useSettingsStore((s) => s.useProxy);
  const setUseProxy = useSettingsStore((s) => s.setUseProxy);
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);
  const [availableVersions, setAvailableVersions] = useState<string[]>([]);
  const [checking, setChecking] = useState(false);
  const [upgrading, setUpgrading] = useState(false);
  const [building, setBuilding] = useState(false);
  const [selectedVersion, setSelectedVersion] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<number | null>(null);
  const [lastResult, setLastResult] = useState<{ success: boolean; message: string } | null>(null);

  useEffect(() => {
    loadVersion();
  }, []);

  const loadVersion = async () => {
    try {
      const version = await getEasyTierVersion();
      setCurrentVersion(version);
    } catch (e) {
      setCurrentVersion(null);
    }
  };

  const handleCheckUpdate = async () => {
    setChecking(true);
    try {
      const versions = await checkEasyTierUpdate();
      setAvailableVersions(versions);
      setLastResult(null);
    } catch (e) {
      setLastResult({ success: false, message: String(e) });
    } finally {
      setChecking(false);
    }
  };

  const handleUpgrade = async (version: string) => {
    setUpgrading(true);
    setLastResult(null);
    setDownloadProgress(0);
    try {
      await upgradeEasyTierWithProgress(version, useProxy, (pct) => {
        setDownloadProgress(pct);
      });
      setDownloadProgress(null);
      setLastResult({ success: true, message: t("settings.upgradedTo", { version }) });
      await loadVersion();
    } catch (e) {
      setDownloadProgress(null);
      setLastResult({ success: false, message: String(e) });
    } finally {
      setUpgrading(false);
    }
  };

  const handleBuildFromSource = async () => {
    setBuilding(true);
    setLastResult(null);
    try {
      const version = await buildEasyTierFromSource();
      setLastResult({ success: true, message: t("settings.builtFromSource", { version }) });
      await loadVersion();
    } catch (e) {
      setLastResult({ success: false, message: String(e) });
    } finally {
      setBuilding(false);
    }
  };

  return (
    <section className="border border-[var(--color-border)] rounded-lg p-4 space-y-3">
      <Flex align="center" gap="2">
        <Cpu size={16} className="text-[var(--color-primary)]" />
        <Text size="2" weight="bold">{t("settings.easytierEngine")}</Text>
      </Flex>

      <div className="grid grid-cols-2 gap-2 text-xs">
        <Text className="text-[var(--color-text-secondary)]">{t("settings.currentVersion")}</Text>
        <Text>{currentVersion ?? t("settings.notInstalled")}</Text>
      </div>

      <Flex align="center" gap="2">
        <Switch
          checked={useProxy}
          onCheckedChange={setUseProxy}
          size="1"
        />
        <Text size="1" className="text-[var(--color-text-secondary)]">
          {t("settings.useProxyDownload")}
        </Text>
      </Flex>

      {downloadProgress !== null && (
        <div className="space-y-1">
          <Progress value={Math.round(downloadProgress)} size="1" />
          <Text size="1" className="text-[var(--color-text-secondary)]">
            {t("settings.downloadProgress", { pct: Math.round(downloadProgress) })}
          </Text>
        </div>
      )}

      <Flex gap="2" wrap="wrap">
        <Button
          onClick={handleCheckUpdate}
          disabled={checking}
          variant="outline"
          size="2"
          loading={checking}
        >
          <RefreshCw size={14} />
          {t("settings.checkUpdate")}
        </Button>
        <Button
          onClick={handleBuildFromSource}
          disabled={building}
          variant="surface"
          size="2"
          loading={building}
        >
          <Hammer size={14} />
          {t("settings.buildFromSource")}
        </Button>
      </Flex>

      {availableVersions.length > 0 && (
        <div className="space-y-2">
          <Text size="1" className="text-[var(--color-text-secondary)]">
            {t("settings.availableVersions")}
          </Text>
          <Flex gap="2">
            <Select.Root
              value={selectedVersion || undefined}
              onValueChange={(v) => setSelectedVersion(v)}
              disabled={upgrading}
              size="2"
            >
              <Select.Trigger className="flex-1" />
              <Select.Content>
                <Select.Group>
                  <Select.Label>{t("settings.availableVersions")}</Select.Label>
                  {availableVersions.map((version) => (
                    <Select.Item
                      key={version}
                      value={version}
                      disabled={version === currentVersion}
                    >
                      {version === currentVersion
                        ? `${version} (${t("settings.current")})`
                        : version}
                    </Select.Item>
                  ))}
                </Select.Group>
              </Select.Content>
            </Select.Root>
            <Button
              onClick={() => selectedVersion && handleUpgrade(selectedVersion)}
              disabled={upgrading || !selectedVersion || selectedVersion === currentVersion}
              variant="solid"
              color="blue"
              size="2"
              loading={upgrading}
            >
              <ArrowUpCircle size={14} />
              {t("settings.upgrade")}
            </Button>
          </Flex>
        </div>
      )}

      {lastResult && (
        <Text size="1" className={lastResult.success ? "text-green-500" : "text-red-500"}>
          {lastResult.message}
        </Text>
      )}
    </section>
  );
}
