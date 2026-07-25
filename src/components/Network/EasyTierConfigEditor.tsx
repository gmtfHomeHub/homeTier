import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { EasyTierConfig, PeerConfig, ProxyNetworkConfig, PortForwardConfig, LogConfig } from "../../types/config";
import { Eye, EyeOff } from "lucide-react";
import { Button, TextField, Checkbox, Text } from "@radix-ui/themes";

interface Props {
  value: Partial<EasyTierConfig>;
  onChange: (value: Partial<EasyTierConfig>) => void;
  title?: string;
  showNetworkIdentity?: boolean;
}

const SECTION_CLASS = "border border-[var(--color-border)] rounded-lg p-4 space-y-3";
const LABEL_CLASS = "block text-xs font-medium text-[var(--color-text-secondary)] mb-1";

export function EasyTierConfigEditor({ value, onChange, title, showNetworkIdentity }: Props) {
  const { t } = useTranslation();
  const [showSecret, setShowSecret] = useState(false);
  const set = (patch: Partial<EasyTierConfig>) => onChange({ ...value, ...patch });
  const setFlags = (patch: Record<string, string | number | boolean>) =>
    onChange({ ...value, flags: { ...value.flags, ...patch } as Record<string, string | number | boolean | bigint> });
  const setNetworkIdentity = (patch: { network_name?: string; network_secret?: string }) => {
    const existing = value.network_identity;
    onChange({
      ...value,
      network_identity: {
        network_name: (patch.network_name ?? existing?.network_name) || "",
        network_secret: patch.network_secret ?? existing?.network_secret,
      },
    });
  };

  return (
    <div className="space-y-4 text-sm">
      {title && <h3 className="font-semibold">{title}</h3>}

      {/* Basic */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">{t("network.basicSettings")}</legend>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className={LABEL_CLASS}>{t("settings.instanceName")}</label>
            <TextField.Root size="1" value={value.instance_name ?? ""} onChange={e => set({ instance_name: e.target.value || undefined })} />
          </div>
          <div>
            <label className={LABEL_CLASS}>{t("settings.hostname")}</label>
            <TextField.Root size="1" value={value.hostname ?? ""} onChange={e => set({ hostname: e.target.value || undefined })} />
          </div>
          <div>
            <label className={LABEL_CLASS}>{t("settings.ipv4")}</label>
            <TextField.Root size="1" value={value.ipv4 ?? ""} onChange={e => set({ ipv4: e.target.value || undefined })} />
          </div>
          <div>
            <label className={LABEL_CLASS}>{t("settings.ipv6")}</label>
            <TextField.Root size="1" value={value.ipv6 ?? ""} onChange={e => set({ ipv6: e.target.value || undefined })} />
          </div>
        </div>
        <Text as="label" size="1" className="flex items-center gap-2">
          <Checkbox checked={value.dhcp ?? false} onCheckedChange={(c) => set({ dhcp: c === true })} />
          {t("network.dhcpAuto")}
        </Text>
      </fieldset>

      {/* Network Identity */}
      {showNetworkIdentity && (
        <fieldset className={SECTION_CLASS}>
          <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">{t("network.networkIdentity")}</legend>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className={LABEL_CLASS}>{t("settings.networkName")}</label>
              <TextField.Root size="1" value={value.network_identity?.network_name ?? ""} onChange={e => setNetworkIdentity({ network_name: e.target.value })} />
            </div>
            <div>
              <label className={LABEL_CLASS}>{t("settings.networkSecret")}</label>
              <TextField.Root size="1" type={showSecret ? "text" : "password"} value={value.network_identity?.network_secret ?? ""} onChange={e => setNetworkIdentity({ network_secret: e.target.value })}>
                <TextField.Slot side="right">
                  <Button type="button" onClick={() => setShowSecret(!showSecret)} variant="ghost" size="1">
                    {showSecret ? <EyeOff size={14} /> : <Eye size={14} />}
                  </Button>
                </TextField.Slot>
              </TextField.Root>
            </div>
          </div>
        </fieldset>
      )}

      {/* Peers */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">{t("network.peersTitle")}</legend>
        {(value.peers ?? []).map((p, i) => (
          <div key={i} className="flex items-start gap-2">
            <TextField.Root size="1" className="flex-1" placeholder="URI" value={p.uri} onChange={e => {
              const peers = [...(value.peers ?? [])];
              peers[i] = { ...peers[i], uri: e.target.value };
              set({ peers });
            }} />
            <Button variant="ghost" color="red" size="1" onClick={() => {
              const peers = (value.peers ?? []).filter((_, j) => j !== i);
              set({ peers: peers.length ? peers : undefined });
            }}>×</Button>
          </div>
        ))}
        <Button variant="ghost" color="blue" size="1" onClick={() => set({ peers: [...(value.peers ?? []), { uri: "" }] })}>{t("network.addPeer")}</Button>
      </fieldset>

      {/* Listeners */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">{t("network.listenersTitle")}</legend>
        {(value.listeners ?? []).map((l, i) => (
          <div key={i} className="flex items-start gap-2">
            <TextField.Root size="1" className="flex-1" placeholder="tcp://0.0.0.0:11000" value={l} onChange={e => {
              const listeners = [...(value.listeners ?? [])];
              listeners[i] = e.target.value;
              set({ listeners });
            }} />
            <Button variant="ghost" color="red" size="1" onClick={() => {
              const listeners = (value.listeners ?? []).filter((_, j) => j !== i);
              set({ listeners: listeners.length ? listeners : undefined });
            }}>×</Button>
          </div>
        ))}
        <Button variant="ghost" color="blue" size="1" onClick={() => set({ listeners: [...(value.listeners ?? []), ""] })}>{t("network.addListener")}</Button>
      </fieldset>

      {/* Proxy Networks */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">{t("network.subnetProxy")}</legend>
        {(value.proxy_networks ?? []).map((pn, i) => (
          <div key={i} className="flex items-start gap-2">
            <TextField.Root size="1" className="flex-1" placeholder="CIDR" value={pn.cidr} onChange={e => {
              const list = [...(value.proxy_networks ?? [])];
              list[i] = { ...list[i], cidr: e.target.value };
              set({ proxy_networks: list });
            }} />
            <Button variant="ghost" color="red" size="1" onClick={() => {
              const list = (value.proxy_networks ?? []).filter((_, j) => j !== i);
              set({ proxy_networks: list.length ? list : undefined });
            }}>×</Button>
          </div>
        ))}
        <Button variant="ghost" color="blue" size="1" onClick={() => set({ proxy_networks: [...(value.proxy_networks ?? []), { cidr: "" }] })}>{t("network.addSubnetProxy")}</Button>
      </fieldset>

      {/* Routes & Exit Nodes */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">{t("network.routesAndExit")}</legend>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className={LABEL_CLASS}>{t("network.routes")}</label>
            <TextField.Root size="1" value={(value.routes ?? []).join(", ")} onChange={e => set({ routes: e.target.value ? e.target.value.split(",").map(s => s.trim()) : undefined })} placeholder="用逗号分隔" />
          </div>
          <div>
            <label className={LABEL_CLASS}>{t("network.exitNodes")}</label>
            <TextField.Root size="1" value={(value.exit_nodes ?? []).join(", ")} onChange={e => set({ exit_nodes: e.target.value ? e.target.value.split(",").map(s => s.trim()) : undefined })} placeholder="用逗号分隔" />
          </div>
        </div>
      </fieldset>

      {/* Flags */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">{t("network.advancedFlags")}</legend>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
          {[
            { key: "mtu", label: t("network.flagMtu"), type: "number" },
            { key: "latency_first", label: t("network.flagLatencyFirst"), type: "checkbox" },
            { key: "enable_kcp_proxy", label: t("network.flagEnableKcp"), type: "checkbox" },
            { key: "enable_quic_proxy", label: t("network.flagEnableQuic"), type: "checkbox" },
            { key: "encryption_algorithm", label: t("network.flagEncryptionAlgorithm"), type: "text" },
            { key: "no_tun", label: t("network.flagNoTun"), type: "checkbox" },
            { key: "disable_p2p", label: t("network.flagDisableP2P"), type: "checkbox" },
            { key: "multi_thread", label: t("network.flagMultiThread"), type: "checkbox" },
            { key: "bind_device", label: t("network.flagBindDevice"), type: "checkbox" },
            { key: "default_protocol", label: t("network.flagDefaultProtocol"), type: "text" },
            { key: "dev_name", label: t("network.flagDevName"), type: "text" },
            { key: "enable_encryption", label: t("network.flagEnableEncryption"), type: "checkbox" },
            { key: "enable_ipv6", label: t("network.flagEnableIPv6"), type: "checkbox" },
          ].map(({ key, label, type }) => (
            <div key={key}>
              {type === "checkbox" ? (
                <Text as="label" size="1" className="flex items-center gap-2">
                  <Checkbox
                    checked={!!(value.flags)?.[key]}
                    onCheckedChange={(c) => setFlags({ [key]: c === true })}
                  />
                  {label}
                </Text>
              ) : (
                <>
                  <label className={LABEL_CLASS}>{label}</label>
                  <TextField.Root size="1" type={type === "number" ? "number" : "text"}
                    value={String((value.flags)?.[key] ?? "")}
                    onChange={e => setFlags({ [key]: type === "number" ? Number(e.target.value) : e.target.value })} />
                </>
              )}
            </div>
          ))}
        </div>
      </fieldset>
    </div>
  );
}