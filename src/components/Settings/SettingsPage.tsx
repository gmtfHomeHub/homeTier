import { useState, useEffect } from "react";
import { LogViewer } from "../Log/LogViewer";
import { EasyTierConfigEditor } from "../Network/EasyTierConfigEditor";
import { TunAuthPanel } from "./TunAuthPanel";
import { EasyTierVersionManager } from "./EasyTierVersionManager";
import { Terminal, Network, Palette, Languages, HelpCircle, Shield } from "lucide-react";
import { getSystemConfig, setSystemConfig, getRelayPrefix, setRelayPrefix, getWebappMode, setWebappMode } from "../../utils/api";
import { useSettingsStore, type SettingsTab } from "../../stores/settingsStore";
import type { NetworkConfig } from "../../types/network";
import { useTranslation } from "react-i18next";
import { Tabs, Tooltip, Button, TextField, Flex, Text } from "@radix-ui/themes";

export function SettingsPage() {
  const { theme, language, setTheme, setLanguage, relayPrefix: storePrefix, setRelayPrefix: setStorePrefix, settingsTab, setSettingsTab } = useSettingsStore();
  const [easytierConfig, setEasytierConfig] = useState<Partial<NetworkConfig>>({});
  const [configLoaded, setConfigLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [relayPrefix, setRelayPrefixState] = useState("");
  const [relayPrefixLoaded, setRelayPrefixLoaded] = useState(false);
  const [webappMode, setWebappModeState] = useState<string | null>(null);
  const { t, i18n } = useTranslation();

  useEffect(() => {
    if (settingsTab === "basic" && !relayPrefixLoaded) {
      getRelayPrefix().then((val) => {
        setRelayPrefixState(val);
        setStorePrefix(val);
        setRelayPrefixLoaded(true);
      });
    }
  }, [settingsTab, relayPrefixLoaded]);

  useEffect(() => {
    if (settingsTab === "basic" && webappMode === null) {
      getWebappMode().then(setWebappModeState);
    }
  }, [settingsTab, webappMode]);

  useEffect(() => {
    if (settingsTab === "easytier" && !configLoaded) {
      getSystemConfig().then((json) => {
        if (json) {
          try { setEasytierConfig(JSON.parse(json)); } catch (err) {
            console.log(err);
          }
        }
        setConfigLoaded(true);
      });
    }
  }, [settingsTab, configLoaded]);

  const handleSave = async () => {
    setSaving(true);
    try {
      await setSystemConfig(JSON.stringify(easytierConfig));
      alert(t("settings.save_success"));
    } catch (e) {
      alert(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleLanguageChange = (lang: "zh" | "zh-TW" | "en") => {
    setLanguage(lang);
    i18n.changeLanguage(lang);
  };

  const tabs: { key: SettingsTab; label: string; icon: React.ReactNode }[] = [
    { key: "basic", label: t("settings.basic"), icon: <Palette size={16} /> },
    { key: "logs", label: t("settings.logs"), icon: <Terminal size={16} /> },
    { key: "easytier", label: t("settings.easytier"), icon: <Network size={16} /> },
  ];

  const themeOptions = [
    { value: "light" as const, label: t("settings.theme_light") },
    { value: "dark" as const, label: t("settings.theme_dark") },
    { value: "system" as const, label: t("settings.theme_system") },
  ];

  const langOptions = [
    { value: "zh" as const, label: t("settings.lang_zh") },
    { value: "zh-TW" as const, label: t("settings.lang_zh_TW") },
    { value: "en" as const, label: t("settings.lang_en") },
  ];

  return (
      <div className="flex flex-col flex-1 min-h-0">
        <Tabs.Root value={settingsTab} onValueChange={(v) => setSettingsTab(v as SettingsTab)} className="flex flex-col flex-1 min-h-0">
          {/* 页签 */}
          <Tabs.List className="flex gap-1 px-4 py-1.5 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
            {tabs.map((tab) => (
              <Tabs.Trigger
                key={tab.key}
                value={tab.key}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-colors data-[state=active]:bg-[var(--color-primary)]/10 data-[state=active]:text-[var(--color-primary)] text-[var(--color-text-secondary)]"
              >
                <Flex align="center" gap="2">
                  {tab.icon}
                  {tab.label}
                </Flex>
              </Tabs.Trigger>
            ))}
          </Tabs.List>

          {/* 内容区 */}
          <Tabs.Content value="basic" forceMount className="data-[state=inactive]:hidden data-[state=active]:flex-1 overflow-y-auto">
            <div className="max-w-lg p-4 space-y-5">
              {/* 主题 */}
              <section>
                <Flex align="center" gap="2" mb="3">
                  <Palette size={16} />
                  <Text size="2" weight="bold">{t("settings.theme")}</Text>
                </Flex>
                <Flex gap="2">
                  {themeOptions.map((opt) => (
                    <Button
                      key={opt.value}
                      onClick={() => setTheme(opt.value)}
                      variant={theme === opt.value ? "solid" : "outline"}
                      color={theme === opt.value ? "blue" : "gray"}
                      size="2"
                      className="flex-1"
                    >
                      {opt.label}
                    </Button>
                  ))}
                </Flex>
              </section>

              {/* 语言 */}
              <section>
                <Flex align="center" gap="2" mb="3">
                  <Languages size={16} />
                  <Text size="2" weight="bold">{t("settings.language")}</Text>
                </Flex>
                <Flex gap="2">
                  {langOptions.map((opt) => (
                    <Button
                      key={opt.value}
                      onClick={() => handleLanguageChange(opt.value)}
                      variant={language === opt.value ? "solid" : "outline"}
                      color={language === opt.value ? "blue" : "gray"}
                      size="2"
                      className="flex-1"
                    >
                      {opt.label}
                    </Button>
                  ))}
                </Flex>
              </section>

              {/* 中继网络前缀 */}
              <section>
                <Flex align="center" gap="2" mb="3">
                  <Network size={16} />
                  <Text size="2" weight="bold">{t("settings.relayPrefix")}</Text>
                  <Tooltip content={t("settings.relayPrefixHelp")}>
                    <span className="inline-flex items-center cursor-pointer text-[var(--color-text-secondary)]">
                      <HelpCircle size={14} />
                    </span>
                  </Tooltip>
                </Flex>
                <TextField.Root
                  value={relayPrefix}
                  onChange={(e) => {
                    const val = e.target.value.replace(/\s+/g, '-');
                    setRelayPrefixState(val);
                  }}
                  onBlur={() => {
                    const final = relayPrefix.trim();
                    setRelayPrefixState(final);
                    setStorePrefix(final);
                    setRelayPrefix(final).catch((e) => alert(String(e)));
                  }}
                  placeholder="homeTier_"
                />
              </section>

              {/* TUN 授权 */}
              <section>
                <Flex align="center" gap="2" mb="3">
                  <Shield size={16} />
                  <Text size="2" weight="bold">{t("settings.tunAuth")}</Text>
                </Flex>
                <TunAuthPanel />
              </section>

              {/* EasyTier 引擎版本 */}
              <section>
                <Flex align="center" gap="2" mb="3">
                  <Network size={16} />
                  <Text size="2" weight="bold">{t("settings.easytierEngine")}</Text>
                </Flex>
                <EasyTierVersionManager />
              </section>

              {/* WebView 模式 */}
              {webappMode !== null && (
                <section>
                  <Flex align="center" gap="2" mb="3">
                    <Text size="2" weight="bold">{t("settings.webappMode")}</Text>
                  </Flex>
                  <Flex gap="2">
                    {([
                      { value: "iframe", label: t("settings.webappModeIframe") },
                      { value: "webview", label: t("settings.webappModeWebview") },
                    ] as const).map((opt) => (
                      <Button
                        key={opt.value}
                        onClick={() => {
                          setWebappModeState(opt.value);
                          setWebappMode(opt.value).catch((e) => alert(String(e)));
                        }}
                        variant={webappMode === opt.value ? "solid" : "outline"}
                        color={webappMode === opt.value ? "blue" : "gray"}
                        size="2"
                        className="flex-1"
                      >
                        {opt.label}
                      </Button>
                    ))}
                  </Flex>
                </section>
              )}
            </div>
          </Tabs.Content>

          <Tabs.Content value="logs" forceMount className="data-[state=inactive]:hidden data-[state=active]:flex data-[state=active]:flex-col data-[state=active]:flex-1 min-h-0 overflow-hidden">
              <LogViewer />
          </Tabs.Content>

          <Tabs.Content value="easytier" forceMount className="data-[state=inactive]:hidden data-[state=active]:flex-1 overflow-y-auto">
            <div className="p-4 space-y-4">
              <EasyTierConfigEditor
                value={easytierConfig}
                onChange={setEasytierConfig}
                title={t("settings.systemConfig")}
              />
              <Flex justify="end" gap="2" pt="2" pb="8">
                <Button
                  onClick={() => {
                    setEasytierConfig({});
                    setSystemConfig(JSON.stringify({}));
                  }}
                  variant="outline"
                  size="2"
                >
                  {t("common.reset")}
                </Button>
                <Button
                  onClick={handleSave}
                  disabled={saving}
                  variant="solid"
                  color="blue"
                  size="2"
                  loading={saving}
                >
                  {saving ? t("common.saving") : t("common.save")}
                </Button>
              </Flex>
            </div>
          </Tabs.Content>
        </Tabs.Root>
      </div>
  );
}