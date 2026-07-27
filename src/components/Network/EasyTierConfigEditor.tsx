import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { NetworkConfig, PortForwardConfig } from "../../types/network";
import { DEFAULT_NETWORK_CONFIG, addRow, removeRow } from "../../types/network";
import { Button, TextField, Checkbox, Text, Select } from "@radix-ui/themes";
import { CollapsibleSection } from "../Common/CollapsibleSection";
import {
  Network, Eye, EyeOff, Plus, Trash2, Globe, Shield, Settings,
} from "lucide-react";

interface Props {
  value: Partial<NetworkConfig>;
  onChange: (value: Partial<NetworkConfig>) => void;
  title?: string;
}

const LABEL_CLASS = "block text-xs font-medium text-[var(--color-text-secondary)] mb-1";
const FIELD_CLASS = "flex flex-col gap-1";

interface BoolFlagDef {
  key: keyof NetworkConfig;
  labelKey: string;
}

const boolFlags: BoolFlagDef[] = [
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
  { key: "enable_socks5", labelKey: "network.flagEnableSocks5" },
  { key: "enable_relay_network_whitelist", labelKey: "network.flagEnableRelayWhitelist" },
  { key: "enable_manual_routes", labelKey: "network.flagEnableManualRoutes" },
];

const protoOptions = ["tcp", "udp"];

export function EasyTierConfigEditor({ value, onChange, title }: Props) {
  const { t } = useTranslation();
  const [showSecret, setShowSecret] = useState(false);

  const set = (patch: Partial<NetworkConfig>) => onChange({ ...value, ...patch });

  const boolVal = (key: keyof NetworkConfig): boolean =>
    value[key] === true;

  const setBool = (key: keyof NetworkConfig, val: boolean) =>
    set({ [key]: val });

  const strVal = (key: keyof NetworkConfig): string =>
    (value[key] ?? "") as string;

  const setStr = (key: keyof NetworkConfig, val: string) =>
    set({ [key]: val || undefined });

  const port_forwards: PortForwardConfig[] = value.port_forwards ?? [];

  const setPortForwards = (pfs: PortForwardConfig[]) => set({ port_forwards: pfs });

  return (
    <div className="space-y-4 text-sm">
      {title && <h3 className="font-semibold">{title}</h3>}

      {/* Panel 1: Basic Settings (always open) */}
      <div className="border border-[var(--color-border)] rounded-lg">
        <div className="flex items-center gap-2 p-4 border-b border-[var(--color-border)]">
          <Globe size={16} />
          <Text size="2" weight="medium">{t("network.basicSettings")}</Text>
        </div>
        <div className="p-4 space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("settings.networkName")}</label>
              <TextField.Root size="1" value={strVal("network_name")}
                onChange={e => setStr("network_name", e.target.value)} />
            </div>
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("settings.networkSecret")}</label>
              <TextField.Root size="1" type={showSecret ? "text" : "password"}
                value={strVal("network_secret")}
                onChange={e => setStr("network_secret", e.target.value)}>
                <TextField.Slot side="right">
                  <Button type="button" onClick={() => setShowSecret(!showSecret)} variant="ghost" size="1">
                    {showSecret ? <EyeOff size={14} /> : <Eye size={14} />}
                  </Button>
                </TextField.Slot>
              </TextField.Root>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("network.virtualIpv4")}</label>
              <TextField.Root size="1" value={strVal("virtual_ipv4")}
                onChange={e => setStr("virtual_ipv4", e.target.value)}
                placeholder="10.0.0.1" />
            </div>
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("network.networkLength")}</label>
              <TextField.Root size="1" type="number"
                value={String(value.network_length ?? 24)}
                onChange={e => set({ network_length: parseInt(e.target.value) || 24 })} />
            </div>
          </div>

          <Text as="label" size="1" className="flex items-center gap-2">
            <Checkbox checked={boolVal("dhcp")}
              onCheckedChange={(c) => setBool("dhcp", c === true)} />
            {t("network.dhcpAuto")}
          </Text>

          <div className={FIELD_CLASS}>
            <label className={LABEL_CLASS}>{t("network.initialNodes")}</label>
            <div className="flex flex-col gap-1">
              {(value.peer_urls ?? []).map((url, i) => (
                <div key={i} className="flex items-start gap-2">
                  <TextField.Root size="1" className="flex-1" value={url}
                    onChange={e => {
                      const urls = [...(value.peer_urls ?? [])];
                      urls[i] = e.target.value;
                      set({ peer_urls: urls });
                    }}
                    placeholder="tcp://:11010" />
                  <Button variant="ghost" color="red" size="1" onClick={() => {
                    const urls = (value.peer_urls ?? []).filter((_, j) => j !== i);
                    set({ peer_urls: urls.length ? urls : [] });
                  }}>×</Button>
                </div>
              ))}
              <Button variant="ghost" color="blue" size="1"
                onClick={() => set({ peer_urls: [...(value.peer_urls ?? []), ""] })}>
                <Plus size={14} className="mr-1" />{t("network.addInitialNode")}
              </Button>
            </div>
          </div>

          <div className={FIELD_CLASS}>
            <label className={LABEL_CLASS}>{t("settings.hostname")}</label>
            <TextField.Root size="1" value={strVal("hostname")}
              onChange={e => setStr("hostname", e.target.value)}
              placeholder={t("network.hostnamePlaceholder")} />
          </div>
        </div>
      </div>

      {/* Panel 2: Advanced Settings (collapsible) */}
      <CollapsibleSection title={t("network.advancedSettings")} defaultOpen={false}>
        <div className="space-y-3">

          {/* Listeners */}
          <div className={FIELD_CLASS}>
            <label className={LABEL_CLASS}>{t("network.listenersTitle")}</label>
            {(value.listener_urls ?? []).map((l, i) => (
              <div key={i} className="flex items-start gap-2">
                <TextField.Root size="1" className="flex-1" value={l}
                  onChange={e => {
                    const urls = [...(value.listener_urls ?? [])];
                    urls[i] = e.target.value;
                    set({ listener_urls: urls });
                  }}
                  placeholder="tcp://0.0.0.0:11010" />
                <Button variant="ghost" color="red" size="1" onClick={() => {
                  const urls = (value.listener_urls ?? []).filter((_, j) => j !== i);
                  set({ listener_urls: urls.length ? urls : [] });
                }}>×</Button>
              </div>
            ))}
            <Button variant="ghost" color="blue" size="1"
              onClick={() => set({ listener_urls: [...(value.listener_urls ?? []), ""] })}>
              <Plus size={14} className="mr-1" />{t("network.addListener")}
            </Button>
          </div>

          {/* Mapped Listeners */}
          <div className={FIELD_CLASS}>
            <label className={LABEL_CLASS}>{t("network.mappedListeners")}</label>
            {(value.mapped_listeners ?? []).map((l, i) => (
              <div key={i} className="flex items-start gap-2">
                <TextField.Root size="1" className="flex-1" value={l}
                  onChange={e => {
                    const urls = [...(value.mapped_listeners ?? [])];
                    urls[i] = e.target.value;
                    set({ mapped_listeners: urls });
                  }} />
                <Button variant="ghost" color="red" size="1" onClick={() => {
                  const urls = (value.mapped_listeners ?? []).filter((_, j) => j !== i);
                  set({ mapped_listeners: urls.length ? urls : [] });
                }}>×</Button>
              </div>
            ))}
            <Button variant="ghost" color="blue" size="1"
              onClick={() => set({ mapped_listeners: [...(value.mapped_listeners ?? []), ""] })}>
              <Plus size={14} className="mr-1" />{t("network.addMappedListener")}
            </Button>
          </div>

          {/* Proxy CIDRs */}
          <div className={FIELD_CLASS}>
            <label className={LABEL_CLASS}>{t("network.subnetProxy")}</label>
            {(value.proxy_cidrs ?? []).map((cidr, i) => (
              <div key={i} className="flex items-start gap-2">
                <TextField.Root size="1" className="flex-1" value={cidr}
                  onChange={e => {
                    const list = [...(value.proxy_cidrs ?? [])];
                    list[i] = e.target.value;
                    set({ proxy_cidrs: list });
                  }}
                  placeholder="10.0.0.0/24" />
                <Button variant="ghost" color="red" size="1" onClick={() => {
                  const list = (value.proxy_cidrs ?? []).filter((_, j) => j !== i);
                  set({ proxy_cidrs: list.length ? list : [] });
                }}>×</Button>
              </div>
            ))}
            <Button variant="ghost" color="blue" size="1"
              onClick={() => set({ proxy_cidrs: [...(value.proxy_cidrs ?? []), ""] })}>
              <Plus size={14} className="mr-1" />{t("network.addSubnetProxy")}
            </Button>
          </div>

          {/* Routes & Exit Nodes */}
          <div className="grid grid-cols-2 gap-3">
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("network.routes")}</label>
              <TextField.Root size="1"
                value={(value.routes ?? []).join(", ")}
                onChange={e => set({ routes: e.target.value ? e.target.value.split(",").map(s => s.trim()) : [] })}
                placeholder={t("network.commaSeparated")} />
            </div>
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("network.exitNodes")}</label>
              <TextField.Root size="1"
                value={(value.exit_nodes ?? []).join(", ")}
                onChange={e => set({ exit_nodes: e.target.value ? e.target.value.split(",").map(s => s.trim()) : [] })}
                placeholder={t("network.commaSeparated")} />
            </div>
          </div>

          {/* Dev Name, MTU, Instance Recv Bps Limit */}
          <div className="grid grid-cols-3 gap-3">
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("network.devName")}</label>
              <TextField.Root size="1" value={strVal("dev_name")}
                onChange={e => setStr("dev_name", e.target.value)}
                placeholder={t("network.devNamePlaceholder")} />
            </div>
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("settings.mtu")}</label>
              <TextField.Root size="1" type="number"
                value={value.mtu != null ? String(value.mtu) : ""}
                onChange={e => set({ mtu: e.target.value ? parseInt(e.target.value) : null })}
                placeholder="1380" />
            </div>
            <div className={FIELD_CLASS}>
              <label className={LABEL_CLASS}>{t("settings.instanceRecvBpsLimit")}</label>
              <TextField.Root size="1" type="number"
                value={value.instance_recv_bps_limit != null ? String(value.instance_recv_bps_limit) : ""}
                onChange={e => set({ instance_recv_bps_limit: e.target.value ? parseInt(e.target.value) : null })}
                placeholder={t("network.unlimited")} />
            </div>
          </div>

          {/* Relay Network Whitelist */}
          <div className={FIELD_CLASS}>
            <label className={LABEL_CLASS}>{t("network.relayNetworkWhitelist")}</label>
            <TextField.Root size="1"
              value={(value.relay_network_whitelist ?? []).join(", ")}
              onChange={e => set({ relay_network_whitelist: e.target.value ? e.target.value.split(",").map(s => s.trim()) : [] })}
              placeholder={t("network.commaSeparated")} />
          </div>

          {/* SOCKS5 */}
          <div className="grid grid-cols-2 gap-3">
            <div className={FIELD_CLASS}>
              <Text as="label" size="1" className="flex items-center gap-2">
                <Checkbox checked={boolVal("enable_socks5")}
                  onCheckedChange={(c) => setBool("enable_socks5", c === true)} />
                {t("network.socks5")}
              </Text>
              {boolVal("enable_socks5") && (
                <TextField.Root size="1" type="number"
                  value={String(value.socks5_port ?? 1080)}
                  onChange={e => set({ socks5_port: parseInt(e.target.value) || 1080 })} />
              )}
            </div>
          </div>

          {/* Boolean flags grid */}
          <div className="pt-2">
            <Text size="1" weight="medium" className="block mb-2">{t("network.flagsSwitch")}</Text>
            <div className="grid grid-cols-2 gap-x-4 gap-y-1 md:grid-cols-3">
              {boolFlags.filter(f => f.key !== "enable_socks5" && f.key !== "enable_manual_routes" && f.key !== "enable_relay_network_whitelist").map(({ key, labelKey }) => (
                <Text as="label" size="1" className="flex items-center gap-2" key={key}>
                  <Checkbox checked={boolVal(key)}
                    onCheckedChange={(c) => setBool(key, c === true)} />
                  {t(labelKey)}
                </Text>
              ))}
            </div>
          </div>
        </div>
      </CollapsibleSection>

      {/* Panel 3: Port Forwards (collapsible) */}
      <CollapsibleSection title={t("network.portForwards")} defaultOpen={false}>
        <div className="space-y-2">
          {port_forwards.map((pf, i) => (
            <div key={i} className="flex items-center gap-2 p-2 border border-[var(--color-border)] rounded">
              <Select.Root value={pf.proto}
                onValueChange={(v) => {
                  const list = [...port_forwards];
                  list[i] = { ...list[i], proto: v };
                  setPortForwards(list);
                }}>
                <Select.Trigger style={{ width: 70 }} />
                <Select.Content>
                  {protoOptions.map(p => (
                    <Select.Item key={p} value={p}>{p.toUpperCase()}</Select.Item>
                  ))}
                </Select.Content>
              </Select.Root>
              <TextField.Root size="1" className="flex-1" value={pf.bind_ip}
                onChange={e => {
                  const list = [...port_forwards];
                  list[i] = { ...list[i], bind_ip: e.target.value };
                  setPortForwards(list);
                }}
                placeholder={t("network.bindAddr")} />
              <Text size="1" color="gray">:</Text>
              <TextField.Root size="1" style={{ width: 70 }} type="number"
                value={String(pf.bind_port)}
                onChange={e => {
                  const list = [...port_forwards];
                  list[i] = { ...list[i], bind_port: parseInt(e.target.value) || 0 };
                  setPortForwards(list);
                }} />
              <Text size="1" color="gray">→</Text>
              <TextField.Root size="1" className="flex-1" value={pf.dst_ip}
                onChange={e => {
                  const list = [...port_forwards];
                  list[i] = { ...list[i], dst_ip: e.target.value };
                  setPortForwards(list);
                }}
                placeholder={t("network.dstAddr")} />
              <Text size="1" color="gray">:</Text>
              <TextField.Root size="1" style={{ width: 70 }} type="number"
                value={String(pf.dst_port)}
                onChange={e => {
                  const list = [...port_forwards];
                  list[i] = { ...list[i], dst_port: parseInt(e.target.value) || 0 };
                  setPortForwards(list);
                }} />
              <Button variant="ghost" color="red" size="1"
                onClick={() => {
                  removeRow(i, port_forwards);
                  setPortForwards([...port_forwards]);
                }}>
                <Trash2 size={14} />
              </Button>
            </div>
          ))}
          <Button variant="ghost" color="blue" size="1"
            onClick={() => {
              addRow(port_forwards);
              setPortForwards([...port_forwards]);
            }}>
            <Plus size={14} className="mr-1" />{t("network.portForwardsAddBtn")}
          </Button>
        </div>
      </CollapsibleSection>

      {/* Panel 4: ACL (collapsible) */}
      <CollapsibleSection title={t("network.acl")} defaultOpen={false}>
        <div className="text-sm text-[var(--color-text-secondary)]">
          {t("network.aclConfigureInTab")}
        </div>
      </CollapsibleSection>
    </div>
  );
}
