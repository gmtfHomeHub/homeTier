import { useState, useEffect } from "react";
import { LogViewer } from "../Log/LogViewer";
import { EasyTierConfigEditor } from "../Network/EasyTierConfigEditor";
import { EasyTierVersionManager } from "./EasyTierVersionManager";
import { AppVersionManager } from "./AppVersionManager";
import { ShortcutEditor } from "./ShortcutEditor";
import { AppConfigEditor } from "./AppConfigEditor";
import { Terminal, Palette, Languages, HelpCircle, Keyboard, FileCog, Network } from "lucide-react";
import { getSystemConfig, setSystemConfig, getLogEnabled, setLogEnabled as setLogEnabledApi } from "../../utils/api";
import { applyGlobalShortcuts } from "../../services/shortcuts";
import { useSettingsStore } from "../../stores/settingsStore";
import type { NetworkConfig } from "../../types/network";
import { useTranslation } from "react-i18next";
import { Tabs, Button, Flex, Text, Switch, Card, Select } from "@radix-ui/themes";
import Tip from "../Common/Tip";
import { SettingTabEnum, LanguageEnum, ThemeEnum } from "../../enum";
import { toastSuccess, toastError } from "../../utils/toast";
import { isMobile } from "../../utils/platform";

export function SettingsPage() {
  const {
    theme,
    language,
    setTheme,
    setLanguage,
    settingsTab: activeTab,
    setSettingsTab,
    logEnabled,
    setLogEnabled: setStoreLogEnabled,
    micShortcut: defMicShortcut,
    speakerShortcut: defSpeakerShortcut,
    setMicShortcut: storeSetMicShortcut,
    setSpeakerShortcut: storeSetSpeakerShortcut,
  } = useSettingsStore();
  // const [activeTab, setActiveTab] = useState<SettingTabEnum>(SettingTabEnum.BASIC);
  const [easytierConfig, setEasytierConfig] = useState<Partial<NetworkConfig>>({});
  const [saving, setSaving] = useState(false);
  const [mobile, setMobile] = useState(false);
  const { t, i18n } = useTranslation();

  useEffect(() => {
    let alive = true;
    isMobile().then((m) => { if (alive) setMobile(m); });
    return () => { alive = false; };
  }, []);

  const setActiveTab = (tab: SettingTabEnum) => {
    // setActiveTab(tab);
    setSettingsTab(tab);
  };

  useEffect(() => {
    getLogEnabled().then((val) => setStoreLogEnabled(val)).catch(() => {});
  }, [setStoreLogEnabled]);

  useEffect(() => {
    if (!logEnabled && activeTab === SettingTabEnum.LOG) {
      setActiveTab(SettingTabEnum.BASIC);
    }
  }, [logEnabled, activeTab]);

  useEffect(() => {
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
      toastSuccess(t("settings.save_success"));
    } catch (e) {
      toastError(String(e));
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
    { key: SettingTabEnum.ET, label: t("settings.easytier"), icon: <Network size={16} /> },
    { key: SettingTabEnum.CONFIG, label: t("settings.config"), icon: <FileCog size={16} /> },
    ...(logEnabled ? [{ key: SettingTabEnum.LOG, label: t("settings.logs"), icon: <Terminal size={16} /> }] : []),
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
          <Tabs.Content value="basic" forceMount className="data-[state=inactive]:hidden data-[state=active]:flex-1 min-h-0 overflow-y-auto">
            <div className="flex flex-col max-w-4xl gap-4 p-4 mx-auto md:flex-row md:items-start">
              {/* 左列 */}
              <div className="flex flex-col flex-1 min-w-0 gap-4">
              {/* 主题 */}
              <Card size="3">
                <Flex align="center" justify="between" gap="3">
                  <Flex align="center" gap="3">
                    <span className="inline-flex items-center justify-center w-9 h-9 rounded-lg bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                      <Palette size={18} />
                    </span>
                    <Flex direction="column">
                      <Text size="3" weight="medium">{t("settings.theme")}</Text>
                      <Text size="1" color="gray">{t("settings.themeDesc")}</Text>
                    </Flex>
                  </Flex>
                    <Select.Root size="1" value={theme} onValueChange={setTheme}>
                      <Select.Trigger />
                      <Select.Content>
                        {themeOptions.map((opt) => (
                          <Select.Item key={opt.value} value={opt.value} title={opt.label}>{opt.label}</Select.Item>

                        ))}
                      </Select.Content>
                    </Select.Root>
                </Flex>
              </Card>

              {/* 语言 */}
              <Card size="3">
                <Flex align="center" justify="between" gap="3">
                  <Flex align="center" gap="3">
                    <span className="inline-flex items-center justify-center w-9 h-9 rounded-lg bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                      <Languages size={18} />
                    </span>
                    <Flex direction="column">
                      <Text size="3" weight="medium">{t("settings.language")}</Text>
                      <Text size="1" color="gray">{t("settings.languageDesc")}</Text>
                    </Flex>
                  </Flex>
                    <Select.Root size="1" value={language} onValueChange={handleLanguageChange}>
                      <Select.Trigger />
                      <Select.Content>
                        {langOptions.map((opt) => (
                          <Select.Item key={opt.value} value={opt.value} title={opt.label}>{opt.label}</Select.Item>

                        ))}
                      </Select.Content>
                    </Select.Root>
                </Flex>
              </Card>

              {/* EasyTier 引擎 / 当前应用（左右两栏） */}
              <Card size="3">
                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <div className="border-r border-[var(--color-border)] md:pr-4" style={{ borderRightStyle: 'solid'}}>
                    <EasyTierVersionManager />
                  </div>
                  <div>
                    <AppVersionManager />
                  </div>
                </div>
              </Card>
              </div>

              {/* 右列 */}
              <div className="flex flex-col flex-1 min-w-0 gap-4">
              {/* 显示日志开关 */}
              <Card size="3">
                <Flex align="center" justify="between" gap="3">
                  <Flex align="center" gap="3">
                    <span className="inline-flex items-center justify-center w-9 h-9 rounded-lg bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                      <Terminal size={18} />
                    </span>
                    <Flex direction="column">
                      <Flex align="center" gap="2">
                        <Text size="3" weight="medium">{t("settings.showLogs")}</Text>
                        <Tip content={t("settings.showLogsHelp")}>
                          <span className="inline-flex items-center cursor-pointer text-[var(--color-text-secondary)]">
                            <HelpCircle size={14} />
                          </span>
                        </Tip>
                      </Flex>
                      <Text size="1" color="gray">{t("settings.showLogsDesc")}</Text>
                    </Flex>
                  </Flex>
                  <Switch
                    size="1"
                    checked={logEnabled}
                    onCheckedChange={(val) => {
                      setStoreLogEnabled(val);
                      setLogEnabledApi(val).catch((e) => toastError(String(e)));
                    }}
                  />
                </Flex>
              </Card>

              {/* 全局快捷键 */}
              <Card size="3">
                <Flex direction="column" gap="3">
                  <Flex align="center" gap="3">
                    <span className="inline-flex items-center justify-center w-9 h-9 rounded-lg bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                      <Keyboard size={18} />
                    </span>
                    <Flex direction="column" className="flex-1">
                      <Flex align="center" gap="2">
                        <Text size="3" weight="medium">{t("settings.shortcuts")}</Text>
                        <Tip content={t("settings.shortcutsHelp")}>
                          <span className="inline-flex items-center cursor-pointer text-[var(--color-text-secondary)]">
                            <HelpCircle size={14} />
                          </span>
                        </Tip>
                      </Flex>
                      <Text size="1" color="gray">{t("settings.shortcutsDesc")}</Text>
                    </Flex>
                  </Flex>
                  {mobile ? (
                    <Text size="1" color="gray" className="pl-12">
                      {t("settings.shortcutUnsupported")}
                    </Text>
                  ) : (
                  <Flex direction="column" gap="3" className="pl-12">
                    <Flex direction="column" gap="1">
                      <Flex align="center" justify="between" gap="2">
                        <Text size="2">{t("settings.micShortcut")}</Text>
                        <ShortcutEditor
                          value={defMicShortcut}
                          onChange={(v) => {
                            storeSetMicShortcut(v);
                            applyGlobalShortcuts().catch((e) => console.error(e));
                          }}
                          placeholder={t("settings.shortcutNotSet")}
                        />
                      </Flex>
                      <Text size="1" color="gray">{t("settings.micShortcutDesc")}</Text>
                    </Flex>
                    <Flex direction="column" gap="1">
                      <Flex align="center" justify="between" gap="2">
                        <Text size="2">{t("settings.speakerShortcut")}</Text>
                        <ShortcutEditor
                          value={defSpeakerShortcut}
                          onChange={(v) => {
                            storeSetSpeakerShortcut(v);
                            applyGlobalShortcuts().catch((e) => console.error(e));
                          }}
                          placeholder={t("settings.shortcutNotSet")}
                        />
                      </Flex>
                      <Text size="1" color="gray">{t("settings.speakerShortcutDesc")}</Text>
                    </Flex>
                    <Flex justify="end" gap="2" mt="1">
                      <Button
                        variant="outline"
                        size="1"
                        onClick={() => {
                          storeSetMicShortcut(defMicShortcut);
                          storeSetSpeakerShortcut(defSpeakerShortcut);
                          applyGlobalShortcuts().catch((e) => console.error(e));
                        }}
                      >
                        {t("common.reset")}
                      </Button>
                    </Flex>
                  </Flex>
                  )}
                </Flex>
              </Card>
              </div>
            </div>
          </Tabs.Content>

          <Tabs.Content value="logs" className="data-[state=inactive]:hidden data-[state=active]:flex data-[state=active]:flex-col data-[state=active]:flex-1 min-h-0 overflow-hidden">
              <LogViewer />
          </Tabs.Content>

          <Tabs.Content value="easytier" forceMount className="data-[state=inactive]:hidden data-[state=active]:flex-1 min-h-0 overflow-y-auto">
            <Card size="3" className="grid max-w-3xl grid-cols-1 gap-4 p-4 mx-auto my-4">
              <EasyTierConfigEditor
                value={easytierConfig}
                onChange={setEasytierConfig}
                // title={t("settings.systemConfig")}
              />
              <Flex justify="end" gap="2" pt="2" pb="8">
                <Button
                  onClick={() => {
                    setEasytierConfig({});
                    setSystemConfig(JSON.stringify({}));
                  }}
                  variant="outline"
                  size="1"
                >
                  {t("common.reset")}
                </Button>
                <Button
                  onClick={handleSave}
                  disabled={saving}
                  variant="solid"
                  color="blue"
                  size="1"
                  loading={saving}
                >
                  {saving ? t("common.saving") : t("common.save")}
                </Button>
              </Flex>
            </Card>
          </Tabs.Content>

          <Tabs.Content value="config" forceMount className="data-[state=inactive]:hidden data-[state=active]:flex-1 min-h-0 overflow-y-auto">
            <Card size="3" className="grid max-w-3xl grid-cols-1 gap-4 p-4 mx-auto my-4">
            <AppConfigEditor />
            </Card>
          </Tabs.Content>
        </Tabs.Root>
      </div>
  );
}