import { useState } from "react";
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
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">基本设置</legend>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className={LABEL_CLASS}>实例名</label>
            <TextField.Root size="1" value={value.instance_name ?? ""} onChange={e => set({ instance_name: e.target.value || undefined })} />
          </div>
          <div>
            <label className={LABEL_CLASS}>主机名</label>
            <TextField.Root size="1" value={value.hostname ?? ""} onChange={e => set({ hostname: e.target.value || undefined })} />
          </div>
          <div>
            <label className={LABEL_CLASS}>IPv4</label>
            <TextField.Root size="1" value={value.ipv4 ?? ""} onChange={e => set({ ipv4: e.target.value || undefined })} />
          </div>
          <div>
            <label className={LABEL_CLASS}>IPv6</label>
            <TextField.Root size="1" value={value.ipv6 ?? ""} onChange={e => set({ ipv6: e.target.value || undefined })} />
          </div>
        </div>
        <Text as="label" size="1" className="flex items-center gap-2">
          <Checkbox checked={value.dhcp ?? false} onCheckedChange={(c) => set({ dhcp: c === true })} />
          DHCP（自动分配 IP）
        </Text>
      </fieldset>

      {/* Network Identity */}
      {showNetworkIdentity && (
        <fieldset className={SECTION_CLASS}>
          <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">网络标识</legend>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className={LABEL_CLASS}>网络名称</label>
              <TextField.Root size="1" value={value.network_identity?.network_name ?? ""} onChange={e => setNetworkIdentity({ network_name: e.target.value })} />
            </div>
            <div>
              <label className={LABEL_CLASS}>网络密钥</label>
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
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">节点 (Peers)</legend>
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
        <Button variant="ghost" color="blue" size="1" onClick={() => set({ peers: [...(value.peers ?? []), { uri: "" }] })}>+ 添加节点</Button>
      </fieldset>

      {/* Listeners */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">监听地址</legend>
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
        <Button variant="ghost" color="blue" size="1" onClick={() => set({ listeners: [...(value.listeners ?? []), ""] })}>+ 添加监听地址</Button>
      </fieldset>

      {/* Proxy Networks */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">子网代理</legend>
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
        <Button variant="ghost" color="blue" size="1" onClick={() => set({ proxy_networks: [...(value.proxy_networks ?? []), { cidr: "" }] })}>+ 添加子网代理</Button>
      </fieldset>

      {/* Routes & Exit Nodes */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">路由 & 出口</legend>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className={LABEL_CLASS}>路由</label>
            <TextField.Root size="1" value={(value.routes ?? []).join(", ")} onChange={e => set({ routes: e.target.value ? e.target.value.split(",").map(s => s.trim()) : undefined })} placeholder="用逗号分隔" />
          </div>
          <div>
            <label className={LABEL_CLASS}>出口节点</label>
            <TextField.Root size="1" value={(value.exit_nodes ?? []).join(", ")} onChange={e => set({ exit_nodes: e.target.value ? e.target.value.split(",").map(s => s.trim()) : undefined })} placeholder="用逗号分隔" />
          </div>
        </div>
      </fieldset>

      {/* Flags */}
      <fieldset className={SECTION_CLASS}>
        <legend className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] px-1">高级标志</legend>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
          {[
            { key: "mtu", label: "MTU", type: "number" },
            { key: "latency_first", label: "低延迟优先", type: "checkbox" },
            { key: "enable_kcp_proxy", label: "启用 KCP", type: "checkbox" },
            { key: "enable_quic_proxy", label: "启用 QUIC", type: "checkbox" },
            { key: "encryption_algorithm", label: "加密算法", type: "text" },
            { key: "no_tun", label: "无 TUN 模式", type: "checkbox" },
            { key: "disable_p2p", label: "禁用 P2P", type: "checkbox" },
            { key: "multi_thread", label: "多线程", type: "checkbox" },
            { key: "bind_device", label: "绑定设备", type: "checkbox" },
            { key: "default_protocol", label: "默认协议", type: "text" },
            { key: "dev_name", label: "设备名", type: "text" },
            { key: "enable_encryption", label: "启用加密", type: "checkbox" },
            { key: "enable_ipv6", label: "启用 IPv6", type: "checkbox" },
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