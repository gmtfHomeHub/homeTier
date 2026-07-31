import { useState, useEffect } from "react";
import { LogViewer } from "../Log/LogViewer";
import { EasyTierConfigEditor } from "../Network/EasyTierConfigEditor";
import { EasyTierVersionManager } from "./EasyTierVersionManager";
import { Terminal, Network, Palette, Languages, HelpCircle } from "lucide-react";
import { getSystemConfig, setSystemConfig, getRelayPrefix, setRelayPrefix, getLogEnabled, setLogEnabled as setLogEnabledApi } from "../../utils/api";
import { useSettingsStore } from "../../stores/settingsStore";
import type { NetworkConfig } from "../../types/network";
import { useTranslation } from "react-i18next";
import { Tabs, Tooltip, Button, TextField, Flex, Text, Switch } from "@radix-ui/themes";
import { SettingTabEnum, LanguageEnum, ThemeEnum } from "../../enum";

export function SettingsPage() {
  const { theme, language, setTheme, setLanguage, relayPrefix: defRelayPrefix, logEnabled, setLogEnabled: setStoreLogEnabled } = useSettingsStore();
  const [activeTab, setActiveTab] = useState<SettingTabEnum>(SettingTabEnum.BASIC);
  const [easytierConfig, setEasytierConfig] = useState<Partial<NetworkConfig>>({});
  const [saving, setSaving] = useState(false);
  const [relayPrefix, setRelayPrefixState] = useState<string | undefined>(defRelayPrefix);
  const { t, i18n } = useTranslation();

  useEffect(() => {
    getLogEnabled().then((val) => setStoreLogEnabled(val)).catch(() => {});
  }, [setStoreLogEnabled]);

  useEffect(() => {
    if (!logEnabled && activeTab === SettingTabEnum.LOG) {
      setActiveTab(SettingTabEnum.BASIC);
    }
  }, [logEnabled, activeTab]);

  useEffect(() => {
    if (activeTab === SettingTabEnum.BASIC) {
      getRelayPrefix().then((val) => {
        setRelayPrefixState(val);
      });
    }

    if (activeTab === SettingTabEnum.ET) {
      getSystemConfig().then((json) => {
        if (json) {
          try { setEasytierConfig(JSON.parse(json)); } catch (err) {
            console.log(err);
          }
        }
      });
    }
  }, [activeTab]);

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

  const handleLanguageChange = (lang: LanguageEnum) => {
    setLanguage(lang);
    i18n.changeLanguage(lang);
  };

  const tabs: { key: SettingTabEnum; label: string; icon: React.ReactNode }[] = [
    { key: SettingTabEnum.BASIC, label: t("settings.basic"), icon: <Palette size={16} /> },
    ...(logEnabled ? [{ key: SettingTabEnum.LOG, label: t("settings.logs"), icon: <Terminal size={16} /> }] : []),
    { key: SettingTabEnum.ET, label: t("settings.easytier"), icon: <Network size={16} /> },
  ];

  const themeOptions = [
    { value: ThemeEnum.LIGHT, label: t("settings.theme_light") },
    { value: ThemeEnum.DARK, label: t("settings.theme_dark") },
    { value: ThemeEnum.SYS, label: t("settings.theme_system") },
  ];

  const langOptions = [
    { value: LanguageEnum.ZH, label: t("settings.lang_zh") },
    { value: LanguageEnum.TW, label: t("settings.lang_zh_TW") },
    { value: LanguageEnum.EN, label: t("settings.lang_en") },
  ];

  return (
      <div className="flex flex-col flex-1 min-h-0">
        <Tabs.Root value={activeTab} onValueChange={(v) => setActiveTab(v as SettingTabEnum)} className="flex flex-col flex-1 min-h-0">
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
                    const final = relayPrefix?.trim() || '';
                    setRelayPrefixState(final);
                    setRelayPrefix(final).catch((e) => alert(String(e)));
                  }}
                  placeholder="homeTier_"
                />
              </section>

              {/* EasyTier 引擎版本 */}
              <section>
                <Flex align="center" gap="2" mb="3">
                  <Network size={16} />
                  <Text size="2" weight="bold">{t("settings.easytierEngine")}</Text>
                </Flex>
                <EasyTierVersionManager />
              </section>

              {/* 显示日志开关 */}
              <section>
                <Flex align="center" justify="between" gap="2">
                  <Flex align="center" gap="2">
                    <Terminal size={16} />
                    <Text size="2" weight="bold">{t("settings.showLogs")}</Text>
                    <Tooltip content={t("settings.showLogsHelp")}>
                      <span className="inline-flex items-center cursor-pointer text-[var(--color-text-secondary)]">
                        <HelpCircle size={14} />
                      </span>
                    </Tooltip>
                  </Flex>
                  <Switch
                    checked={logEnabled}
                    onCheckedChange={(val) => {
                      setStoreLogEnabled(val);
                      setLogEnabledApi(val).catch((e) => alert(String(e)));
                    }}
                  />
                </Flex>
              </section>
            </div>
          </Tabs.Content>

          <Tabs.Content value="logs" className="data-[state=inactive]:hidden data-[state=active]:flex data-[state=active]:flex-col data-[state=active]:flex-1 min-h-0 overflow-hidden">
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