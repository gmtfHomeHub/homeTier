import React, { useState, useEffect } from "react";
import { Flex, Text, Button, TextField, Switch, ScrollArea, Card } from "@radix-ui/themes";
import { Tabs } from "@radix-ui/themes";
import { Network, Shield, Settings, HelpCircle, Check, X, Save, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSpace } from "../../hooks/useSpace";
import { useToast } from "../../hooks/useToast";
import { NetworkConfigDetails } from "../../types";
import { updateLocalConfig } from "../../utils/api";

interface NetworkConfigEditorProps {
  spaceId: string;
}

export const NetworkConfigEditor: React.FC<NetworkConfigEditorProps> = ({ spaceId }) => {
  const { t } = useTranslation();
  const { space, loading, error } = useSpace(spaceId);
  const { showToast } = useToast();
  
  const [config, setConfig] = useState<NetworkConfigDetails>({
    space_id: spaceId,
    network_name: "",
    network_secret: "",
    dhcp: false,
    peers: [],
    listeners: [],
    mapped_listeners: [],
    proxy_networks: [],
    routes: [],
    exit_nodes: [],
    port_forwards: [],
    flags: {},
  });
  
  const [isSaving, setIsSaving] = useState(false);
  const [showSuccess, setShowSuccess] = useState(false);

  useEffect(() => {
    if (space) {
        setConfig({
          space_id: spaceId,
          network_name: space.network_name,
          network_secret: space.network_secret,
          dhcp: false, // 从 space 对象中获取默认值
          peers: [],
          listeners: [],
          mapped_listeners: [],
          proxy_networks: [],
          routes: [],
          exit_nodes: [],
          port_forwards: [],
          flags: {},
        });
    }
  }, [space]);

  const handleSave = async () => {
    setIsSaving(true);
    try {
      await updateLocalConfig(spaceId, config);
      setShowSuccess(true);
      showToast({
        title: t("settings.configSaved"),
        variant: "success",
      });
      setTimeout(() => setShowSuccess(false), 3000);
    } catch (err) {
      showToast({
        title: t("settings.configSaveError"),
        description: String(err),
        variant: "error",
      });
    } finally {
      setIsSaving(false);
    }
  };

  const handleReset = () => {
    if (space) {
      setConfig({
        space_id: spaceId,
        network_name: space.network_name,
        network_secret: space.network_secret,
        dhcp: false,
        peers: [],
        listeners: [],
        mapped_listeners: [],
        proxy_networks: [],
        routes: [],
        exit_nodes: [],
        port_forwards: [],
        flags: {},
      });
    }
  };

  if (loading) {
    return (
      <Flex justify="center" align="center" height="200px">
        <Text>{t("common.loading")}</Text>
      </Flex>
    );
  }

  if (error) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-lg p-4">
        <div className="text-red-800 font-medium">{t("settings.error")}</div>
        <div className="text-red-600 text-sm">{error}</div>
      </div>
    );
  }

  return (
    <Card>
      <div className="flex items-center gap-2 p-4 border-b border-[var(--color-border)]">
        <Text size="2" weight="bold">{t("settings.localConfig")}</Text>
      </div>
      <div className="p-4">
        {showSuccess && (
          <div className="bg-green-50 border border-green-200 rounded-lg p-4 mb-4">
            <Text size="1" weight="bold" className="text-green-800">{t("settings.configSaved")}</Text>
          </div>
        )}

        <Tabs.Root defaultValue="basic">
          <Tabs.List>
            <Tabs.Trigger value="basic">{t("settings.basic")}</Tabs.Trigger>
            <Tabs.Trigger value="advanced">{t("settings.advanced")}</Tabs.Trigger>
          </Tabs.List>

          <Tabs.Content value="basic" className="space-y-4">
            <Flex direction="column" gap="4">
              {/* 网络名称 */}
              <Flex direction="column" gap="2">
                <label className="text-sm font-medium flex items-center gap-2">
                  <Network size={16} />
                  {t("settings.networkName")}
                </label>
                <TextField.Root
                  value={config.network_name}
                  onChange={(e) => setConfig({ ...config, network_name: e.target.value })}
                  placeholder={t("settings.networkNamePlaceholder")}
                />
              </Flex>

              {/* 网络密钥 */}
              <Flex direction="column" gap="2">
                <label className="text-sm font-medium flex items-center gap-2">
                  <Shield size={16} />
                  {t("settings.networkSecret")}
                </label>
                <TextField.Root
                  type="password"
                  value={config.network_secret}
                  onChange={(e) => setConfig({ ...config, network_secret: e.target.value })}
                  placeholder={t("settings.networkSecretPlaceholder")}
                />
              </Flex>

              {/* DHCP */}
              <Flex align="center" gap="2">
                <Switch
                  checked={config.dhcp}
                  onCheckedChange={(checked) => setConfig({ ...config, dhcp: checked })}
                />
                <label className="text-sm">{t("settings.dhcp")}</label>
              </Flex>

              {/* 操作按钮 */}
              <Flex gap="2" justify="end">
                <Button variant="outline" onClick={handleReset}>
                  <RefreshCw size={16} className="mr-2" />
                  {t("settings.reset")}
                </Button>
                <Button onClick={handleSave} disabled={isSaving}>
                  {isSaving ? (
                    <RefreshCw size={16} className="animate-spin mr-2" />
                  ) : (
                    <Save size={16} className="mr-2" />
                  )}
                  {t("settings.save")}
                </Button>
              </Flex>
            </Flex>
          </Tabs.Content>

          <Tabs.Content value="advanced" className="space-y-4">
            <ScrollArea className="h-96">
              <Flex direction="column" gap="4">
                {/* 高级配置项 */}
                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.instanceName")}</label>
                  <TextField.Root
                    value={config.instance_name || ""}
                    onChange={(e) => setConfig({ ...config, instance_name: e.target.value })}
                    placeholder={t("settings.instanceNamePlaceholder")}
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.hostname")}</label>
                  <TextField.Root
                    value={config.hostname || ""}
                    onChange={(e) => setConfig({ ...config, hostname: e.target.value })}
                    placeholder={t("settings.hostnamePlaceholder")}
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.ipv4")}</label>
                  <TextField.Root
                    value={config.ipv4 || ""}
                    onChange={(e) => setConfig({ ...config, ipv4: e.target.value })}
                    placeholder="192.168.1.100"
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.ipv6")}</label>
                  <TextField.Root
                    value={config.ipv6 || ""}
                    onChange={(e) => setConfig({ ...config, ipv6: e.target.value })}
                    placeholder="fd00::1"
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.ipv6PublicAddrProvider")}</label>
                  <Switch
                    checked={config.ipv6_public_addr_provider || false}
                    onCheckedChange={(checked) => setConfig({ ...config, ipv6_public_addr_provider: checked })}
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.ipv6PublicAddrAuto")}</label>
                  <Switch
                    checked={config.ipv6_public_addr_auto || false}
                    onCheckedChange={(checked) => setConfig({ ...config, ipv6_public_addr_auto: checked })}
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.ipv6PublicAddrPrefix")}</label>
                  <TextField.Root
                    value={config.ipv6_public_addr_prefix || ""}
                    onChange={(e) => setConfig({ ...config, ipv6_public_addr_prefix: e.target.value })}
                    placeholder="fd00::/64"
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.peers")}</label>
                  <TextField.Root
                    value={config.peers.map(p => p.uri).join(", ")}
                    onChange={(e) => {
                      const uris = e.target.value.split(",").map(s => s.trim()).filter(s => s);
                      const peers = uris.map(uri => ({ uri, peer_public_key: "" }));
                      setConfig({ ...config, peers });
                    }}
                    placeholder="tcp://peer1.example.com:10000, tcp://peer2.example.com:10000"
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.listeners")}</label>
                  <TextField.Root
                    value={config.listeners.join(", ")}
                    onChange={(e) => {
                      const listeners = e.target.value.split(",").map(s => s.trim()).filter(s => s);
                      setConfig({ ...config, listeners });
                    }}
                    placeholder="tcp://0.0.0.0:10000, udp://0.0.0.0:10000"
                  />
                </Flex>

                <Flex direction="column" gap="2">
                  <label className="text-sm font-medium">{t("settings.flags")}</label>
                  <TextField.Root
                    value={Object.entries(config.flags)
                      .map(([k, v]) => `${k}=${v}`)
                      .join(", ")}
                    onChange={(e) => {
                      const flags: Record<string, string> = {};
                      const flagEntries = e.target.value.split(",").map(s => s.trim()).filter(s => s);
                      for (const entry of flagEntries) {
                        const [key, value] = entry.split("=");
                        if (key && value) {
                          flags[key.trim()] = value.trim();
                        }
                      }
                      setConfig({ ...config, flags });
                    }}
                    placeholder="enable_kcp_proxy=true, mtu=1400"
                  />
                </Flex>
              </Flex>
            </ScrollArea>

            <Flex gap="2" justify="end" mt="4">
              <Button variant="outline" onClick={handleReset}>
                <RefreshCw size={16} className="mr-2" />
                {t("settings.reset")}
              </Button>
              <Button onClick={handleSave} disabled={isSaving}>
                {isSaving ? (
                  <RefreshCw size={16} className="animate-spin mr-2" />
                ) : (
                  <Save size={16} className="mr-2" />
                )}
                {t("settings.save")}
              </Button>
            </Flex>
          </Tabs.Content>
        </Tabs.Root>
      </div>
    </Card>
  );
};