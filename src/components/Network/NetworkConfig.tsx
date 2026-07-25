import { useState, useEffect } from "react";
import { Button, Checkbox, Text, TextField, Card, Flex, Badge } from "@radix-ui/themes";
import { AlertDialog, Select } from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { useToast, ToastHelpers } from "../../hooks/useToast";
import { Globe, Shield, Sliders, Plus, Trash2, Edit2, ExternalLink, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AclRule, PortForwardRule } from "../../types";
import { 
  getAclRules, 
  createAclRule, 
  updateAclRule, 
  deleteAclRule,
  getPortForwardRules, 
  createPortForwardRule, 
  updatePortForwardRule, 
  deletePortForwardRule,
} from "../../utils/api";

export function NetworkConfig() {
  const { spaces, currentSpaceId } = useSpaceStore();
  const space = spaces.find((s) => s.id === currentSpaceId);
  const { showToast } = useToast();
  const [activeTab, setActiveTab] = useState<"basic" | "advanced" | "acl" | "forwarding">("basic");
  const { t } = useTranslation();

  if (!space) {
    return (
      <div className="text-center py-8 text-[var(--color-text-secondary)] text-sm">
        {t("common.selectSpace")}
      </div>
    );
  }

  return (
    <div className="bg-[var(--color-surface)] rounded-xl border border-[var(--color-border)]">
      {/* 标签页 */}
      <div className="flex border-b border-[var(--color-border)]">
        <Button
          onClick={() => setActiveTab("basic")}
          variant={activeTab === "basic" ? "solid" : "ghost"}
          color="blue"
          size="2"
          className="flex-1"
        >
          <Globe size={16} />
          {t("network.basic")}
        </Button>
        <Button
          onClick={() => setActiveTab("advanced")}
          variant={activeTab === "advanced" ? "solid" : "ghost"}
          color="blue"
          size="2"
          className="flex-1"
        >
          <Sliders size={16} />
          {t("network.advanced")}
        </Button>
        <Button
          onClick={() => setActiveTab("acl")}
          variant={activeTab === "acl" ? "solid" : "ghost"}
          color="blue"
          size="2"
          className="flex-1"
        >
          <Shield size={16} />
          {t("network.acl")}
        </Button>
        <Button
          onClick={() => setActiveTab("forwarding")}
          variant={activeTab === "forwarding" ? "solid" : "ghost"}
          color="blue"
          size="2"
          className="flex-1"
        >
          <ExternalLink size={16} />
          {t("network.portForwarding")}
        </Button>
      </div>

      {/* 内容 */}
      <div className="p-4">
        {activeTab === "basic" && (
          <div className="space-y-3">
            <div>
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">
                {t("network.networkName")}
              </label>
              <div className="mt-1 font-mono text-sm">{space.network_name}</div>
            </div>
            <div>
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">
                {t("network.virtualIp")}
              </label>
              <div className="mt-1 font-mono text-sm">
                {space.virtual_ip || t("network.dhcpAutoAssign")}
              </div>
            </div>
            <div>
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">
                {t("network.connectionStatus")}
              </label>
              <div className="mt-1 text-sm">
                <span
                  className={`inline-flex items-center gap-1 ${
                    space.status === "connected"
                      ? "text-[var(--color-success)]"
                      : space.status === "connecting"
                      ? "text-yellow-400"
                      : "text-[var(--color-text-secondary)]"
                  }`}
                >
                  <span className="w-2 h-2 bg-current rounded-full" />
                  {space.status === "connected"
                    ? t("network.connected")
                    : space.status === "connecting"
                    ? t("network.connecting")
                    : t("network.disconnected")}
                </span>
              </div>
            </div>
          </div>
        )}

        {activeTab === "advanced" && (
          <div className="text-sm text-[var(--color-text-secondary)]">
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <Text size="2">{t("network.kcpProxy")}</Text>
                <Checkbox />
              </div>
              <div className="flex items-center justify-between">
                <Text size="2">{t("network.quicProxy")}</Text>
                <Checkbox />
              </div>
              <div className="flex items-center justify-between">
                <Text size="2">{t("network.latencyFirst")}</Text>
                <Checkbox />
              </div>
            </div>
          </div>
        )}

        {activeTab === "acl" && (
          <AclConfig spaceId={space.id} showToast={showToast} />
        )}

        {activeTab === "forwarding" && (
          <PortForwardingConfig spaceId={space.id} showToast={showToast} />
        )}
      </div>
    </div>
  );
}

// ACL 配置组件
function AclConfig({ spaceId, showToast }: { spaceId: string; showToast: ToastHelpers['showToast'] }) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<AclRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [isAddingRule, setIsAddingRule] = useState(false);
  const [editingRule, setEditingRule] = useState<AclRule | null>(null);
  const [newRule, setNewRule] = useState<{action: "allow" | "deny"; source: string; dest: string; ports: string; description: string}>({ action: "allow", source: "any", dest: "any", ports: "1-65535", description: "" });

  useEffect(() => {
    setLoading(true);
    getAclRules(spaceId)
      .then(setRules)
      .catch((e) => showToast({ title: t("network.loadFailed"), variant: "error" }))
      .finally(() => setLoading(false));
  }, [spaceId]);

  const handleSaveRule = async () => {
    try {
      if (editingRule) {
        const updated = await updateAclRule(spaceId, editingRule.id, newRule.action, newRule.source, newRule.dest, newRule.ports, newRule.description);
        setRules(rules.map(r => r.id === editingRule.id ? updated : r));
        setEditingRule(null);
      } else {
        const created = await createAclRule(spaceId, newRule.action, newRule.source, newRule.dest, newRule.ports, newRule.description);
        setRules([...rules, created]);
      }
      setNewRule({ action: "allow", source: "any", dest: "any", ports: "1-65535", description: "" });
      setIsAddingRule(false);
      showToast({ title: t("network.aclSaved"), variant: "success" });
    } catch (e) {
      showToast({ title: t("network.saveFailed"), variant: "error" });
    }
  };

  const handleDeleteRule = async (id: string) => {
    try {
      await deleteAclRule(spaceId, id);
      setRules(rules.filter(r => r.id !== id));
      showToast({ title: t("network.aclDeleted"), variant: "success" });
    } catch (e) {
      showToast({ title: t("network.deleteFailed"), variant: "error" });
    }
  };

  const handleEditRule = (rule: AclRule) => {
    setEditingRule(rule);
    setNewRule({
      action: rule.action as "allow" | "deny",
      source: rule.source,
      dest: rule.dest,
      ports: rule.ports,
      description: rule.description,
    });
    setIsAddingRule(true);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">{t("network.aclTitle")}</h3>
        <Button onClick={() => setIsAddingRule(true)}>
          <Plus size={16} />
          {t("network.addRule")}
        </Button>
      </div>

      <div className="border rounded-lg">
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="p-3 text-left">{t("network.aclTableAction")}</th>
              <th className="p-3 text-left">{t("network.aclTableSource")}</th>
              <th className="p-3 text-left">{t("network.aclTableDest")}</th>
              <th className="p-3 text-left">{t("network.aclTablePort")}</th>
              <th className="p-3 text-left">{t("network.aclTableDescription")}</th>
              <th className="p-3 text-left">{t("network.aclTableAction")}</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={6} className="p-6 text-center">
                  <Loader2 className="inline animate-spin mr-2" size={16} />
                  {t("common.loading")}
                </td>
              </tr>
            ) : rules.length === 0 ? (
              <tr>
                <td colSpan={6} className="p-6 text-center text-sm text-[var(--color-text-secondary)]">
                  {t("network.aclEmpty")}
                </td>
              </tr>
            ) : rules.map((rule) => (
              <tr key={rule.id} className="border-b">
                <td className="p-3">
                  <span className={`px-2 py-1 rounded text-xs ${
                    rule.action === "allow" 
                      ? "bg-green-100 text-green-800" 
                      : "bg-red-100 text-red-800"
                  }`}>
                    {rule.action === "allow" ? t("network.aclActionAllow") : t("network.aclActionDeny")}
                  </span>
                </td>
                <td className="p-3 font-mono text-sm">{rule.source}</td>
                <td className="p-3 font-mono text-sm">{rule.dest}</td>
                <td className="p-3 font-mono text-sm">{rule.ports}</td>
                <td className="p-3 text-sm">{rule.description}</td>
                <td className="p-3">
                  <Flex gap="2">
                    <Button size="1" variant="ghost" onClick={() => handleEditRule(rule)}>
                      <Edit2 size={14} />
                    </Button>
                    <AlertDialog.Root>
                      <AlertDialog.Trigger>
                        <Button size="1" variant="ghost" color="red">
                          <Trash2 size={14} />
                        </Button>
                      </AlertDialog.Trigger>
                      <AlertDialog.Content>
                        <AlertDialog.Title>{t("network.aclDeleteTitle")}</AlertDialog.Title>
                        <AlertDialog.Description>
                          {t("network.aclConfirmDelete")}
                        </AlertDialog.Description>
                        <AlertDialog.Action onClick={() => handleDeleteRule(rule.id)}>
                          {t("common.delete")}
                        </AlertDialog.Action>
                      </AlertDialog.Content>
                    </AlertDialog.Root>
                  </Flex>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {(isAddingRule || editingRule) && (
        <Card>
          <div className="p-4 border-b border-[var(--color-border)]">
            <Text size="2" weight="bold">{editingRule ? t("network.aclEditRule") : t("network.aclAddRuleForm")}</Text>
          </div>
          <div className="p-4 space-y-4">
            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.aclTableAction")}</label>
              <Select.Root 
                value={newRule.action} 
                onValueChange={(value) => setNewRule({ ...newRule, action: value as "allow" | "deny" })}
              >
                <Select.Trigger>
                  <div>{newRule.action === "allow" ? t("network.aclActionAllow") : t("network.aclActionDeny")}</div>
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="allow">{t("network.aclActionAllow")}</Select.Item>
                  <Select.Item value="deny">{t("network.aclActionDeny")}</Select.Item>
                </Select.Content>
              </Select.Root>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.aclSourceIp")}</label>
              <TextField.Root
                value={newRule.source}
                onChange={(e) => setNewRule({ ...newRule, source: e.target.value })}
                placeholder="any, 192.168.1.0/24, 10.0.0.1"
              />
              <Text size="1" color="gray">{t("network.aclCidrHelp")}</Text>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.aclDestIp")}</label>
              <TextField.Root
                value={newRule.dest}
                onChange={(e) => setNewRule({ ...newRule, dest: e.target.value })}
                placeholder="any, 192.168.100.10, 10.0.0.0/8"
              />
              <Text size="1" color="gray">{t("network.aclCidrHelp")}</Text>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.port")}</label>
              <TextField.Root
                value={newRule.ports}
                onChange={(e) => setNewRule({ ...newRule, ports: e.target.value })}
                placeholder="any, 80, 443, 1000-2000"
              />
              <Text size="1" color="gray">{t("network.portHelp")}</Text>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.description")}</label>
              <TextField.Root
                value={newRule.description}
                onChange={(e) => setNewRule({ ...newRule, description: e.target.value })}
                placeholder={t("network.aclDescriptionPlaceholder")}
              />
            </Flex>

            <Flex gap="2" justify="end">
              <Button variant="outline" onClick={() => { setIsAddingRule(false); setEditingRule(null); }}>
                {t("common.cancel")}
              </Button>
              <Button onClick={handleSaveRule}>
                {t("common.save")}
              </Button>
            </Flex>
          </div>
        </Card>
      )}
    </div>
  );
}

// 端口转发配置组件
function PortForwardingConfig({ spaceId, showToast }: { spaceId: string; showToast: ToastHelpers['showToast'] }) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<PortForwardRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [isAddingRule, setIsAddingRule] = useState(false);
  const [editingRule, setEditingRule] = useState<PortForwardRule | null>(null);
  const [newRule, setNewRule] = useState<{name: string; protocol: "tcp" | "udp"; sourceIp: string; sourcePort: number; targetIp: string; targetPort: number; description: string}>({ 
    name: "", 
    protocol: "tcp", 
    sourceIp: "any", 
    sourcePort: 8080, 
    targetIp: "192.168.100.1", 
    targetPort: 80, 
    description: "" 
  });

  useEffect(() => {
    setLoading(true);
    getPortForwardRules(spaceId)
      .then(setRules)
      .catch(() => showToast({ title: t("network.loadFailed"), variant: "error" }))
      .finally(() => setLoading(false));
  }, [spaceId]);

  const handleSaveRule = async () => {
    try {
      if (editingRule) {
        const updated = await updatePortForwardRule(spaceId, editingRule.id, newRule.name, newRule.protocol, newRule.sourceIp, newRule.sourcePort, newRule.targetIp, newRule.targetPort, newRule.description);
        setRules(rules.map(r => r.id === editingRule.id ? updated : r));
        setEditingRule(null);
      } else {
        const created = await createPortForwardRule(spaceId, newRule.name, newRule.protocol, newRule.sourceIp, newRule.sourcePort, newRule.targetIp, newRule.targetPort, newRule.description);
        setRules([...rules, created]);
      }
      setNewRule({ 
        name: "", 
        protocol: "tcp", 
        sourceIp: "any", 
        sourcePort: 8080, 
        targetIp: "192.168.100.1", 
        targetPort: 80, 
        description: "" 
      });
      setIsAddingRule(false);
      showToast({ title: t("network.portForwardSaved"), variant: "success" });
    } catch {
      showToast({ title: t("network.saveFailed"), variant: "error" });
    }
  };

  const handleDeleteRule = async (id: string) => {
    try {
      await deletePortForwardRule(spaceId, id);
      setRules(rules.filter(r => r.id !== id));
      showToast({ title: t("network.portForwardDeleted"), variant: "success" });
    } catch {
      showToast({ title: t("network.deleteFailed"), variant: "error" });
    }
  };

  const handleEditRule = (rule: PortForwardRule) => {
    setEditingRule(rule);
    setNewRule({
      name: rule.name,
      protocol: rule.protocol as "tcp" | "udp",
      sourceIp: rule.source_ip,
      sourcePort: rule.source_port,
      targetIp: rule.target_ip,
      targetPort: rule.target_port,
      description: rule.description,
    });
    setIsAddingRule(true);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">{t("network.portForwardTitle")}</h3>
        <Button onClick={() => setIsAddingRule(true)}>
          <Plus size={16} />
          {t("network.portForwardAddRule")}
        </Button>
      </div>

      <div className="border rounded-lg">
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="p-3 text-left">{t("network.name")}</th>
              <th className="p-3 text-left">{t("network.protocol")}</th>
              <th className="p-3 text-left">{t("network.source")}</th>
              <th className="p-3 text-left">{t("network.destination")}</th>
              <th className="p-3 text-left">{t("network.description")}</th>
              <th className="p-3 text-left">{t("network.aclTableAction")}</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={6} className="p-6 text-center">
                  <Loader2 className="inline animate-spin mr-2" size={16} />
                  {t("common.loading")}
                </td>
              </tr>
            ) : rules.length === 0 ? (
              <tr>
                <td colSpan={6} className="p-6 text-center text-sm text-[var(--color-text-secondary)]">
                  {t("network.portForwardEmpty")}
                </td>
              </tr>
            ) : rules.map((rule) => (
              <tr key={rule.id} className="border-b">
                <td className="p-3">
                  <Text size="2" weight="medium">{rule.name}</Text>
                </td>
                <td className="p-3">
                  <Badge variant="outline">{rule.protocol.toUpperCase()}</Badge>
                </td>
                <td className="p-3">
                  <Text size="1">
                    {rule.source_ip}:{rule.source_port}
                  </Text>
                </td>
                <td className="p-3">
                  <Text size="1">
                    {rule.target_ip}:{rule.target_port}
                  </Text>
                </td>
                <td className="p-3 text-sm">{rule.description}</td>
                <td className="p-3">
                  <Flex gap="2">
                    <Button size="1" variant="ghost" onClick={() => handleEditRule(rule)}>
                      <Edit2 size={14} />
                    </Button>
                    <AlertDialog.Root>
                      <AlertDialog.Trigger>
                        <Button size="1" variant="ghost" color="red">
                          <Trash2 size={14} />
                        </Button>
                      </AlertDialog.Trigger>
                      <AlertDialog.Content>
                        <AlertDialog.Title>{t("network.portForwardDeleteTitle")}</AlertDialog.Title>
                        <AlertDialog.Description>
                          {t("network.portForwardConfirmDelete")}
                        </AlertDialog.Description>
                        <AlertDialog.Action onClick={() => handleDeleteRule(rule.id)}>
                          {t("common.delete")}
                        </AlertDialog.Action>
                      </AlertDialog.Content>
                    </AlertDialog.Root>
                  </Flex>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {(isAddingRule || editingRule) && (
        <Card>
          <div className="p-4 border-b border-[var(--color-border)]">
            <Text size="2" weight="bold">{editingRule ? t("network.portForwardEditRule") : t("network.portForwardAddRuleForm")}</Text>
          </div>
          <div className="p-4 space-y-4">
            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.ruleName")}</label>
              <TextField.Root
                value={newRule.name}
                onChange={(e) => setNewRule({ ...newRule, name: e.target.value })}
                placeholder={t("network.portForwardNamePlaceholder")}
              />
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.protocol")}</label>
              <Select.Root 
                value={newRule.protocol} 
                onValueChange={(value) => setNewRule({ ...newRule, protocol: value as "tcp" | "udp" })}
              >
                <Select.Trigger>
                  <div>{newRule.protocol.toUpperCase()}</div>
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="tcp">{t("network.tcp")}</Select.Item>
                  <Select.Item value="udp">{t("network.udp")}</Select.Item>
                </Select.Content>
              </Select.Root>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.portForwardSourceListen")}</label>
              <Flex gap="2">
                <Flex direction="column" gap="1" className="flex-1">
                  <Text size="1" color="gray">{t("network.source")}</Text>
                  <TextField.Root
                    value={newRule.sourceIp}
                    onChange={(e) => setNewRule({ ...newRule, sourceIp: e.target.value })}
                    placeholder="any, 192.168.100.0/24"
                  />
                </Flex>
                <Flex direction="column" gap="1" className="w-20">
                  <Text size="1" color="gray">{t("network.port")}</Text>
                  <TextField.Root
                    type="number"
                    value={newRule.sourcePort.toString()}
                    onChange={(e) => setNewRule({ ...newRule, sourcePort: parseInt(e.target.value) || 0 })}
                    placeholder="8080"
                  />
                </Flex>
              </Flex>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.portForwardTargetAddr")}</label>
              <Flex gap="2">
                <Flex direction="column" gap="1" className="flex-1">
                  <Text size="1" color="gray">{t("network.destination")}</Text>
                  <TextField.Root
                    value={newRule.targetIp}
                    onChange={(e) => setNewRule({ ...newRule, targetIp: e.target.value })}
                    placeholder="192.168.100.10"
                  />
                </Flex>
                <Flex direction="column" gap="1" className="w-20">
                  <Text size="1" color="gray">{t("network.port")}</Text>
                  <TextField.Root
                    type="number"
                    value={newRule.targetPort.toString()}
                    onChange={(e) => setNewRule({ ...newRule, targetPort: parseInt(e.target.value) || 0 })}
                    placeholder="80"
                  />
                </Flex>
              </Flex>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">{t("network.description")}</label>
              <TextField.Root
                value={newRule.description}
                onChange={(e) => setNewRule({ ...newRule, description: e.target.value })}
                placeholder={t("network.portForwardDescriptionPlaceholder")}
              />
            </Flex>

            <Flex gap="2" justify="end">
              <Button variant="outline" onClick={() => { setIsAddingRule(false); setEditingRule(null); }}>
                {t("common.cancel")}
              </Button>
              <Button onClick={handleSaveRule}>
                {t("common.save")}
              </Button>
            </Flex>
          </div>
        </Card>
      )}
    </div>
  );
}