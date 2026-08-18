import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getEasyTierVersion, checkEasyTierUpdate, upgradeEasyTierWithProgress, getAppConfig } from "../../utils/api";
import { Button, Text, Flex, Select, Switch, Progress } from "@radix-ui/themes";
import { Cpu, RefreshCw, ArrowUpCircle } from "lucide-react";
import { useSettingsStore } from "../../stores/settingsStore";
import { toastSuccess, toastError } from "../../utils/toast";
import { isMobile } from "../../utils/platform";

export function EasyTierVersionManager() {
  const { t } = useTranslation();
  const useProxy = useSettingsStore((s) => s.useProxy);
  const setUseProxy = useSettingsStore((s) => s.setUseProxy);
  const [mobile, setMobile] = useState(false);
  const [githubMirror, setGithubMirror] = useState<string>("");
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);
  const [availableVersions, setAvailableVersions] = useState<string[]>([]);
  const [checking, setChecking] = useState(false);
  const [upgrading, setUpgrading] = useState(false);
  const [selectedVersion, setSelectedVersion] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<number | null>(null);

  useEffect(() => {
    loadVersion();
  }, []);

  // 移动端不支持在线升级（引擎随应用编译），隐藏升级入口
  useEffect(() => {
    let alive = true;
    isMobile().then((m) => { if (alive) setMobile(m); });
    return () => { alive = false; };
  }, []);

  // 读取 GITHUB_MIRROR 配置：非空才展示代理下载开关
  useEffect(() => {
    getAppConfig()
      .then((cfg) => setGithubMirror((cfg["GITHUB_MIRROR"] ?? "").trim()))
      .catch(() => setGithubMirror(""));
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
      // 仅显示 >= 当前版本的版本
      const filtered = currentVersion
        ? versions.filter((v) => compareVersions(v, currentVersion) >= 0)
        : versions;
      setSelectedVersion(currentVersion || null);
      setAvailableVersions(filtered);
    } catch (e) {
      toastError(String(e));
    } finally {
      setChecking(false);
    }
  };

  // 语义化版本比较：返回 1 (a>b), 0 (a=b), -1 (a<b)
  const compareVersions = (a: string, b: string): number => {
    const parse = (v: string) => v.replace(/^v/, "").split(".").map(Number);
    const pa = parse(a);
    const pb = parse(b);
    for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
      const na = pa[i] ?? 0;
      const nb = pb[i] ?? 0;
      if (na > nb) return 1;
      if (na < nb) return -1;
    }
    return 0;
  };

  const handleUpgrade = async (version: string) => {
    setUpgrading(true);
    setDownloadProgress(0);
    try {
      await upgradeEasyTierWithProgress(version, useProxy, (pct) => {
        setDownloadProgress(pct);
      });
      setDownloadProgress(null);
      toastSuccess(t("settings.upgradedTo", { version }));
      await loadVersion();
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
        <Cpu size={16} className="text-[var(--color-primary)]" />
        <Text size="2" weight="bold">{t("settings.easytierEngine")}</Text>
      </Flex>

      <Flex align="center" justify="between">
        <div className="grid grid-cols-2 gap-2 text-xs">
          <Text className="text-[var(--color-text-secondary)]">{t("settings.currentVersion")}</Text>
          <Text>{currentVersion ?? t("settings.notInstalled")}</Text>
        </div>

      <Flex gap="2" wrap="wrap">
        {!mobile && (
        <Button
          onClick={handleCheckUpdate}
          disabled={checking}
          variant="ghost"
          size="1"
          loading={checking}
        >
          <ArrowUpCircle size={14} />
          {t("settings.update")}
        </Button>
        )}
      </Flex>
      </Flex>

      {githubMirror && (
        <Flex align="center" gap="2">
          <Switch
            checked={useProxy}
            onCheckedChange={setUseProxy}
            size="1"
          />
          <Text size="1" className="text-[var(--color-text-secondary)]">
            {t("settings.useProxyDownload", { mirror: githubMirror })}
          </Text>
        </Flex>
      )}

      {downloadProgress !== null && (
        <div className="space-y-1">
          <Progress value={Math.round(downloadProgress)} size="1" />
          <Text size="1" className="text-[var(--color-text-secondary)]">
            {t("settings.downloadProgress", { pct: Math.round(downloadProgress) })}
          </Text>
        </div>
      )}


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
    </section>
  );
}
