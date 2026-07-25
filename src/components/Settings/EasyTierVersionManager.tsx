import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getEasyTierVersion, checkEasyTierUpdate, upgradeEasyTier } from "../../utils/api";
import { Button, Text, Flex } from "@radix-ui/themes";
import { Cpu, RefreshCw, ArrowUpCircle, CheckCircle } from "lucide-react";

export function EasyTierVersionManager() {
  const { t } = useTranslation();
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);
  const [availableVersions, setAvailableVersions] = useState<string[]>([]);
  const [checking, setChecking] = useState(false);
  const [upgrading, setUpgrading] = useState(false);
  const [selectedVersion, setSelectedVersion] = useState<string | null>(null);
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
    setSelectedVersion(version);
    setLastResult(null);
    try {
      await upgradeEasyTier(version);
      setLastResult({ success: true, message: t("settings.upgradedTo", { version }) });
      await loadVersion();
    } catch (e) {
      setLastResult({ success: false, message: String(e) });
    } finally {
      setUpgrading(false);
      setSelectedVersion(null);
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

      <Flex gap="2">
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
      </Flex>

      {availableVersions.length > 0 && (
        <div className="space-y-2">
          <Text size="1" className="text-[var(--color-text-secondary)]">
            {t("settings.availableVersions")}
          </Text>
          <div className="flex flex-wrap gap-2">
            {availableVersions.map((version) => (
              <Button
                key={version}
                onClick={() => handleUpgrade(version)}
                disabled={upgrading || version === currentVersion}
                variant={version === currentVersion ? "solid" : "outline"}
                color={version === currentVersion ? "green" : "blue"}
                size="1"
                loading={upgrading && selectedVersion === version}
              >
                {version === currentVersion ? (
                  <><CheckCircle size={12} /> {t("settings.current")}</>
                ) : (
                  <><ArrowUpCircle size={12} /> {t("settings.upgrade")}</>
                )}
              </Button>
            ))}
          </div>
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
