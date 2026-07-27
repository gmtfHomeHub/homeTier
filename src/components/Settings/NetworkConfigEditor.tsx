import React, { useState, useEffect } from "react";
import { Flex, Text, Button, TextField, Switch, ScrollArea, Card } from "@radix-ui/themes";
import { Network, Shield, Settings, HelpCircle, Check, X, Save, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSpace } from "../../hooks/useSpace";
import { useToast } from "../../hooks/useToast";
import type { NetworkConfig } from "../../types/network";
import { DEFAULT_NETWORK_CONFIG } from "../../types/network";
import { updateLocalConfig } from "../../utils/api";
import { CollapsibleSection } from "../Common/CollapsibleSection";

interface NetworkConfigEditorProps {
  spaceId: string;
}

const boolFlagKeys: { key: keyof NetworkConfig; labelKey: string }[] = [
  { key: "latency_first", labelKey: "network.flagLatencyFirst" },
  { key: "use_smoltcp", labelKey: "network.flagUseSmoltcp" },
  { key: "disable_ipv6", labelKey: "network.flagDisableIpv6" },
  { key: "ipv6_public_addr_auto", labelKey: "network.flagIpv6PublicAddrAuto" },
  { key: "enable_kcp_proxy", labelKey: "network.flagEnableKcp" },
  { key: "disable_kcp_input", labelKey: "network.flagDisableKcpInput" },
  { key: "enable_quic_proxy", labelKey: "network.flagEnableQuic" },
  { key: "disable_quic_input", labelKey: "network.flagDisableQuicInput" },
  { key: "disable_p2p", labelKey: "network.flagDisableP2P" },
  { key: "p2p_only", labelKey: "network.flagP2pOnly" },
  { key: "lazy_p2p", labelKey: "network.flagLazyP2p" },
  { key: "bind_device", labelKey: "network.flagBindDevice" },
  { key: "no_tun", labelKey: "network.flagNoTun" },
  { key: "enable_exit_node", labelKey: "network.flagEnableExitNode" },
  { key: "relay_all_peer_rpc", labelKey: "network.flagRelayAllPeerRpc" },
  { key: "need_p2p", labelKey: "network.flagNeedP2p" },
  { key: "multi_thread", labelKey: "network.flagMultiThread" },
  { key: "proxy_forward_by_system", labelKey: "network.flagProxyForwardBySystem" },
  { key: "disable_encryption", labelKey: "network.flagDisableEncryption" },
  { key: "disable_tcp_hole_punching", labelKey: "network.flagDisableTcpHolePunch" },
  { key: "disable_udp_hole_punching", labelKey: "network.flagDisableUdpHolePunch" },
  { key: "disable_upnp", labelKey: "network.flagDisableUpnp" },
  { key: "enable_udp_broadcast_relay", labelKey: "network.flagEnableUdpBroadcast" },
  { key: "disable_sym_hole_punching", labelKey: "network.flagDisableSymHolePunch" },
  { key: "enable_magic_dns", labelKey: "network.flagEnableMagicDns" },
  { key: "enable_private_mode", labelKey: "network.flagEnablePrivateMode" },
];

export const NetworkConfigEditor: React.FC<NetworkConfigEditorProps> = ({ spaceId }) => {
  const { t } = useTranslation();
  const { space, loading, error } = useSpace(spaceId);
  const { showToast } = useToast();
  
  const [config, setConfig] = useState<NetworkConfig>(DEFAULT_NETWORK_CONFIG());
  
  const [isSaving, setIsSaving] = useState(false);
  const [showSuccess, setShowSuccess] = useState(false);

  useEffect(() => {
    if (space) {
      if (space.config_json) {
        try {
          const parsed = JSON.parse(space.config_json) as Partial<NetworkConfig>;
          setConfig({ ...DEFAULT_NETWORK_CONFIG(), ...parsed, network_name: space.network_name, network_secret: space.network_secret });
        } catch {
          setConfig({ ...DEFAULT_NETWORK_CONFIG(), network_name: space.network_name, network_secret: space.network_secret });
        }
      } else {
        setConfig({ ...DEFAULT_NETWORK_CONFIG(), network_name: space.network_name, network_secret: space.network_secret });
      }
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
      setConfig({ ...DEFAULT_NETWORK_CONFIG(), network_name: space.network_name, network_secret: space.network_secret });
    }
  };

  const setBool = (key: keyof NetworkConfig, val: boolean) =>
    setConfig({ ...config, [key]: val } as any);

  const setStr = (key: keyof NetworkConfig, val: string) =>
    setConfig({ ...config, [key]: val } as any);

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
      <div className="p-4 space-y-4">
        {showSuccess && (
          <div className="bg-green-50 border border-green-200 rounded-lg p-4 mb-4">
            <Text size="1" weight="bold" className="text-green-800">{t("settings.configSaved")}</Text>
          </div>
        )}

        <div className="border border-[var(--color-border)] rounded-lg">
          <div className="flex items-center gap-2 p-4 border-b border-[var(--color-border)]">
            <Network size={16} />
            <Text size="2" weight="medium">{t("settings.basic")}</Text>
          </div>
          <div className="p-4 space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <Flex direction="column" gap="1">
                <label className="text-xs font-medium">{t("settings.networkName")}</label>
                <TextField.Root value={config.network_name}
                  onChange={(e) => setStr("network_name", e.target.value)} />
              </Flex>
              <Flex direction="column" gap="1">
                <label className="text-xs font-medium">{t("settings.networkSecret")}</label>
                <TextField.Root type="password" value={config.network_secret}
                  onChange={(e) => setStr("network_secret", e.target.value)} />
              </Flex>
            </div>
            <Flex direction="column" gap="1">
              <label className="text-xs font-medium">{t("network.virtualIpv4")}</label>
              <TextField.Root value={config.virtual_ipv4}
                onChange={(e) => setStr("virtual_ipv4", e.target.value)} />
            </Flex>
            <Flex align="center" gap="2">
              <Switch checked={config.dhcp}
                onCheckedChange={(c) => setBool("dhcp", c)} />
              <label className="text-sm">{t("settings.dhcp")}</label>
            </Flex>
            <Flex direction="column" gap="1">
              <label className="text-xs font-medium">{t("network.initialNodes")}</label>
              <TextField.Root value={config.peer_urls.join(", ")}
                onChange={(e) => setConfig({ ...config, peer_urls: e.target.value.split(",").map(s => s.trim()).filter(s => s) })}
                placeholder="tcp://:11010" />
            </Flex>
          </div>
        </div>

        <CollapsibleSection title={t("settings.advanced")} defaultOpen={false}>
          <div className="space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <Flex direction="column" gap="1">
                <label className="text-xs font-medium">{t("settings.instanceName")}</label>
                <TextField.Root value={config.instance_id}
                  onChange={(e) => setStr("instance_id", e.target.value)} />
              </Flex>
              <Flex direction="column" gap="1">
                <label className="text-xs font-medium">{t("settings.hostname")}</label>
                <TextField.Root value={config.hostname ?? ""}
                  onChange={(e) => setStr("hostname", e.target.value)} />
              </Flex>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <Flex direction="column" gap="1">
                <label className="text-xs font-medium">{t("settings.listeners")}</label>
                <TextField.Root value={config.listener_urls.join(", ")}
                  onChange={(e) => setConfig({ ...config, listener_urls: e.target.value.split(",").map(s => s.trim()).filter(s => s) })} />
              </Flex>
              <Flex direction="column" gap="1">
                <label className="text-xs font-medium">{t("network.subnetProxy")}</label>
                <TextField.Root value={config.proxy_cidrs.join(", ")}
                  onChange={(e) => setConfig({ ...config, proxy_cidrs: e.target.value.split(",").map(s => s.trim()).filter(s => s) })} />
              </Flex>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <Flex direction="column" gap="1">
                <label className="text-xs font-medium">{t("settings.mtu")}</label>
                <TextField.Root type="number" value={config.mtu != null ? String(config.mtu) : ""}
                  onChange={(e) => setConfig({ ...config, mtu: e.target.value ? parseInt(e.target.value) : null })} />
              </Flex>
              <Flex direction="column" gap="1">
                <label className="text-xs font-medium">{t("network.socks5")}</label>
                <TextField.Root type="number" value={String(config.socks5_port)}
                  onChange={(e) => setConfig({ ...config, socks5_port: parseInt(e.target.value) || 1080 })} />
              </Flex>
            </div>
            <div>
              <Text size="1" weight="medium" className="mb-2 block">{t("settings.flags")}</Text>
              <div className="grid grid-cols-2 md:grid-cols-3 gap-2">
                {boolFlagKeys.map(({ key, labelKey }) => (
                  <label key={key as string} className="flex items-center gap-2 text-sm">
                    <Switch checked={(config as any)[key] === true}
                      onCheckedChange={(c) => setBool(key, c)} />
                    {t(labelKey)}
                  </label>
                ))}
              </div>
            </div>
          </div>
        </CollapsibleSection>

        <CollapsibleSection title={t("settings.portForwards")} defaultOpen={false}>
          <div className="text-sm text-[var(--color-text-secondary)]">
            {config.port_forwards.length === 0 ? t("settings.noPortForwards") : (
              <div className="space-y-2">
                {config.port_forwards.map((pf, i) => (
                  <div key={i} className="flex items-center gap-2 text-xs">
                    <span className="font-mono">{pf.proto}://{pf.bind_ip}:{pf.bind_port} → {pf.dst_ip}:{pf.dst_port}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </CollapsibleSection>

        <Flex gap="2" justify="end" pt="2">
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
      </div>
    </Card>
  );
};
