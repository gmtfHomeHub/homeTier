import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getAppConfig,
  setAppConfig,
  getConfigFilePath,
  getConfigTemplatePath,
} from "../../utils/api";
import { listen } from "@tauri-apps/api/event";
import { Button, TextField, Text, Flex, Callout } from "@radix-ui/themes";
import { FileCog, RefreshCw, AlertTriangle } from "lucide-react";

interface KeyMeta {
  description: string;
  default: string;
  enum?: string;
}

/** 键元数据（与后端配置模板保持一致） */
const KEY_META: Record<string, KeyMeta> = {
  DAEMON_IPC_PORT: { description: "config.descDaemonIpcPort", default: "15889", enum: "1-65535" },
  EASYTIER_RPC_PORT: { description: "config.descEasytierRpcPort", default: "15888", enum: "1-65535" },
  FILE_SERVER_PORT_BASE: { description: "config.descFileServerPortBase", default: "19000", enum: "1-65535" },
  DEFAULT_SPACE_IP: { description: "config.descDefaultSpaceIp", default: "10.144.144.10", enum: "IPv4" },
  GITHUB_API: { description: "config.descGithubApi", default: "https://api.github.com/repos/EasyTier/EasyTier/releases", enum: "URL" },
  GITHUB_MIRROR: { description: "config.descGithubMirror", default: "https://ghproxy.top", enum: "URL" },
  RELAY_NETWORK_PREFIX: { description: "config.descRelayPrefix", default: "homeTier_", enum: "string" },
  LOG_ENABLED: { description: "config.descLogEnabled", default: "1", enum: "1=on, 0=off" },
};

const PORT_KEYS = new Set(["DAEMON_IPC_PORT", "EASYTIER_RPC_PORT"]);

export function AppConfigEditor() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<Record<string, string>>({});
  const [path, setPath] = useState("");
  const [templatePath, setTemplatePath] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const load = async () => {
    try {
      const [cfg, cfgPath, tmplPath] = await Promise.all([
        getAppConfig(),
        getConfigFilePath(),
        getConfigTemplatePath(),
      ]);
      setConfig(cfg);
      setPath(cfgPath);
      setTemplatePath(tmplPath);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    load();
    const unlisten = listen("config:changed", () => load());
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleSave = async () => {
    setSaving(true);
    setSaved(false);
    try {
      await setAppConfig(config);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      alert(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleReset = async () => {
    const next: Record<string, string> = {};
    for (const key of Object.keys(config)) {
      next[key] = KEY_META[key]?.default ?? config[key];
    }
    setConfig(next);
  };

  return (
    <div className="p-4 space-y-4">
      <Flex align="center" gap="2" mb="2">
        <FileCog size={16} />
        <Text size="2" weight="bold">{t("config.title")}</Text>
      </Flex>

      <Callout.Root size="1" variant="outline">
        <Callout.Icon>
          <FileCog size={14} />
        </Callout.Icon>
        <Callout.Text>
          <Text as="span" size="1">{t("config.path")}: <code>{path}</code></Text>
        </Callout.Text>
      </Callout.Root>

      {templatePath && (
        <Callout.Root size="1" variant="outline" color="gray">
          <Callout.Icon>
            <FileCog size={14} />
          </Callout.Icon>
          <Callout.Text>
            <Text as="span" size="1">{t("config.templatePath")}: <code>{templatePath}</code></Text>
          </Callout.Text>
        </Callout.Root>
      )}

      <Callout.Root size="1" variant="soft" color="amber">
        <Callout.Icon>
          <AlertTriangle size={14} />
        </Callout.Icon>
        <Callout.Text>
          <Text as="span" size="1">{t("config.restartHint")}</Text>
        </Callout.Text>
      </Callout.Root>

      <div className="space-y-2">
        {Object.keys(config).sort().map((key) => {
          const meta = KEY_META[key];
          const isPort = PORT_KEYS.has(key);
          return (
            <div key={key} className="border border-[var(--color-border)] rounded-lg p-3 space-y-1.5">
              <Flex align="center" justify="between" gap="2">
                <Text size="2" weight="bold">{key}</Text>
                {isPort && (
                  <Text size="1" className="text-amber-500">{t("config.portNeedsRestart")}</Text>
                )}
              </Flex>
              {meta && (
                <Text size="1" className="text-[var(--color-text-secondary)]">
                  {t(meta.description)}
                  {meta.enum && <span className="ml-2 text-[var(--color-text-tertiary)]">[{meta.enum}]</span>}
                  <span className="ml-2">({t("config.default")}: {meta.default})</span>
                </Text>
              )}
              <TextField.Root
                value={config[key] ?? ""}
                onChange={(e) => setConfig((prev) => ({ ...prev, [key]: e.target.value }))}
                size="2"
              />
            </div>
          );
        })}
      </div>

      <Flex justify="end" gap="2" pt="2" pb="8">
        <Button variant="outline" size="2" onClick={handleReset}>
          <RefreshCw size={14} />
          {t("common.reset")}
        </Button>
        <Button
          variant="solid"
          color="blue"
          size="2"
          onClick={handleSave}
          disabled={saving}
          loading={saving}
        >
          {saved ? t("config.saved") : (saving ? t("common.saving") : t("common.save"))}
        </Button>
      </Flex>
    </div>
  );
}
