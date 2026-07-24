import { useState, useEffect } from "react";
import { Button, Checkbox, Text, TextField, Switch, Card, ScrollArea, Flex, Badge } from "@radix-ui/themes";
import { AlertDialog, Dialog, Select } from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { useToast, ToastHelpers } from "../../hooks/useToast";
import { Settings, Globe, Shield, Sliders, Plus, Trash2, Edit2, ExternalLink, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
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
                网络名称
              </label>
              <div className="mt-1 font-mono text-sm">{space.network_name}</div>
            </div>
            <div>
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">
                虚拟 IP
              </label>
              <div className="mt-1 font-mono text-sm">
                {space.virtual_ip || "DHCP 自动分配"}
              </div>
            </div>
            <div>
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">
                连接状态
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
                    ? "已连接"
                    : space.status === "connecting"
                    ? "连接中"
                    : "未连接"}
                </span>
              </div>
            </div>
          </div>
        )}

        {activeTab === "advanced" && (
          <div className="text-sm text-[var(--color-text-secondary)]">
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <Text size="2">KCP 代理</Text>
                <Checkbox />
              </div>
              <div className="flex items-center justify-between">
                <Text size="2">QUIC 代理</Text>
                <Checkbox />
              </div>
              <div className="flex items-center justify-between">
                <Text size="2">延迟优先模式</Text>
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
  const [rules, setRules] = useState<Array<{id: string; action: "allow" | "deny"; source: string; dest: string; ports: string; description: string}>>([
    { id: "1", action: "allow", source: "any", dest: "192.168.100.0/24", ports: "1-65535", description: "允许本地子网访问" },
    { id: "2", action: "deny", source: "10.0.0.0/8", dest: "any", ports: "any", description: "拒绝内部网络访问" },
  ]);
  const [isAddingRule, setIsAddingRule] = useState(false);
  const [editingRule, setEditingRule] = useState<{id: string; action: "allow" | "deny"; source: string; dest: string; ports: string; description: string} | null>(null);
  const [newRule, setNewRule] = useState<{action: "allow" | "deny"; source: string; dest: string; ports: string; description: string}>({ action: "allow", source: "any", dest: "any", ports: "1-65535", description: "" });

  const handleSaveRule = () => {
    if (editingRule) {
      setRules(rules.map(r => r.id === editingRule.id ? { ...newRule, id: editingRule.id } : r));
      setEditingRule(null);
    } else {
      setRules([...rules, { ...newRule, id: Date.now().toString() }]);
    }
    setNewRule({ action: "allow", source: "any", dest: "any", ports: "1-65535", description: "" });
    setIsAddingRule(false);
    showToast({ title: "ACL 规则已保存", variant: "success" });
  };

  const handleDeleteRule = (id: string) => {
    setRules(rules.filter(r => r.id !== id));
    showToast({ title: "ACL 规则已删除", variant: "success" });
  };

  const handleEditRule = (rule: {id: string; action: "allow" | "deny"; source: string; dest: string; ports: string; description: string}) => {
    setEditingRule(rule);
    setNewRule({
      action: rule.action,
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
        <h3 className="text-lg font-semibold">访问控制列表 (ACL)</h3>
        <Button onClick={() => setIsAddingRule(true)}>
          <Plus size={16} />
          添加规则
        </Button>
      </div>

      <div className="border rounded-lg">
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="p-3 text-left">操作</th>
              <th className="p-3 text-left">来源</th>
              <th className="p-3 text-left">目标</th>
              <th className="p-3 text-left">端口</th>
              <th className="p-3 text-left">描述</th>
              <th className="p-3 text-left">操作</th>
            </tr>
          </thead>
          <tbody>
            {rules.map((rule) => (
              <tr key={rule.id} className="border-b">
                <td className="p-3">
                  <span className={`px-2 py-1 rounded text-xs ${
                    rule.action === "allow" 
                      ? "bg-green-100 text-green-800" 
                      : "bg-red-100 text-red-800"
                  }`}>
                    {rule.action === "allow" ? "允许" : "拒绝"}
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
                        <AlertDialog.Title>删除规则</AlertDialog.Title>
                        <AlertDialog.Description>
                          确定要删除这条 ACL 规则吗？
                        </AlertDialog.Description>
                        <AlertDialog.Action onClick={() => handleDeleteRule(rule.id)}>
                          删除
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
            <Text size="2" weight="bold">{editingRule ? "编辑 ACL 规则" : "添加 ACL 规则"}</Text>
          </div>
          <div className="p-4 space-y-4">
            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">操作</label>
              <Select.Root 
                value={newRule.action} 
                onValueChange={(value) => setNewRule({ ...newRule, action: value as "allow" | "deny" })}
              >
                <Select.Trigger>
                  <div>{newRule.action === "allow" ? "允许" : "拒绝"}</div>
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="allow">允许 (Allow)</Select.Item>
                  <Select.Item value="deny">拒绝 (Deny)</Select.Item>
                </Select.Content>
              </Select.Root>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">来源 IP</label>
              <TextField.Root
                value={newRule.source}
                onChange={(e) => setNewRule({ ...newRule, source: e.target.value })}
                placeholder="any, 192.168.1.0/24, 10.0.0.1"
              />
              <Text size="1" color="gray">支持 CIDR 格式，使用 "any" 表示所有 IP</Text>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">目标 IP</label>
              <TextField.Root
                value={newRule.dest}
                onChange={(e) => setNewRule({ ...newRule, dest: e.target.value })}
                placeholder="any, 192.168.100.10, 10.0.0.0/8"
              />
              <Text size="1" color="gray">支持 CIDR 格式，使用 "any" 表示所有 IP</Text>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">端口</label>
              <TextField.Root
                value={newRule.ports}
                onChange={(e) => setNewRule({ ...newRule, ports: e.target.value })}
                placeholder="any, 80, 443, 1000-2000"
              />
              <Text size="1" color="gray">单个端口、端口范围或 "any"</Text>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">描述</label>
              <TextField.Root
                value={newRule.description}
                onChange={(e) => setNewRule({ ...newRule, description: e.target.value })}
                placeholder="规则的用途说明"
              />
            </Flex>

            <Flex gap="2" justify="end">
              <Button variant="outline" onClick={() => { setIsAddingRule(false); setEditingRule(null); }}>
                取消
              </Button>
              <Button onClick={handleSaveRule}>
                保存
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
  const [rules, setRules] = useState<Array<{id: string; name: string; protocol: "tcp" | "udp"; sourceIp: string; sourcePort: number; targetIp: string; targetPort: number; description: string}>>([
    { id: "1", name: "Web服务", protocol: "tcp", sourceIp: "any", sourcePort: 8080, targetIp: "192.168.100.10", targetPort: 80, description: "转发到内部Web服务器" },
    { id: "2", name: "数据库", protocol: "tcp", sourceIp: "192.168.100.0/24", sourcePort: 3306, targetIp: "192.168.100.20", targetPort: 3306, description: "MySQL数据库访问" },
  ]);
  const [isAddingRule, setIsAddingRule] = useState(false);
  const [editingRule, setEditingRule] = useState<{id: string; name: string; protocol: "tcp" | "udp"; sourceIp: string; sourcePort: number; targetIp: string; targetPort: number; description: string} | null>(null);
  const [newRule, setNewRule] = useState<{name: string; protocol: "tcp" | "udp"; sourceIp: string; sourcePort: number; targetIp: string; targetPort: number; description: string}>({ 
    name: "", 
    protocol: "tcp", 
    sourceIp: "any", 
    sourcePort: 8080, 
    targetIp: "192.168.100.1", 
    targetPort: 80, 
    description: "" 
  });

  const handleSaveRule = () => {
    if (editingRule) {
      setRules(rules.map(r => r.id === editingRule.id ? { ...newRule, id: editingRule.id } : r));
      setEditingRule(null);
    } else {
      setRules([...rules, { ...newRule, id: Date.now().toString() }]);
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
    showToast({ title: "端口转发规则已保存", variant: "success" });
  };

  const handleDeleteRule = (id: string) => {
    setRules(rules.filter(r => r.id !== id));
    showToast({ title: "端口转发规则已删除", variant: "success" });
  };

  const handleEditRule = (rule: {id: string; name: string; protocol: "tcp" | "udp"; sourceIp: string; sourcePort: number; targetIp: string; targetPort: number; description: string}) => {
    setEditingRule(rule);
    setNewRule({
      name: rule.name,
      protocol: rule.protocol,
      sourceIp: rule.sourceIp,
      sourcePort: rule.sourcePort,
      targetIp: rule.targetIp,
      targetPort: rule.targetPort,
      description: rule.description,
    });
    setIsAddingRule(true);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">端口转发</h3>
        <Button onClick={() => setIsAddingRule(true)}>
          <Plus size={16} />
          添加规则
        </Button>
      </div>

      <div className="border rounded-lg">
        <table className="w-full">
          <thead>
            <tr className="border-b">
              <th className="p-3 text-left">名称</th>
              <th className="p-3 text-left">协议</th>
              <th className="p-3 text-left">来源</th>
              <th className="p-3 text-left">目标</th>
              <th className="p-3 text-left">描述</th>
              <th className="p-3 text-left">操作</th>
            </tr>
          </thead>
          <tbody>
            {rules.map((rule) => (
              <tr key={rule.id} className="border-b">
                <td className="p-3">
                  <Text size="2" weight="medium">{rule.name}</Text>
                </td>
                <td className="p-3">
                  <Badge variant="outline">{rule.protocol.toUpperCase()}</Badge>
                </td>
                <td className="p-3">
                  <Text size="1">
                    {rule.sourceIp}:{rule.sourcePort}
                  </Text>
                </td>
                <td className="p-3">
                  <Text size="1">
                    {rule.targetIp}:{rule.targetPort}
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
                        <AlertDialog.Title>删除规则</AlertDialog.Title>
                        <AlertDialog.Description>
                          确定要删除这条端口转发规则吗？
                        </AlertDialog.Description>
                        <AlertDialog.Action onClick={() => handleDeleteRule(rule.id)}>
                          删除
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
            <Text size="2" weight="bold">{editingRule ? "编辑端口转发规则" : "添加端口转发规则"}</Text>
          </div>
          <div className="p-4 space-y-4">
            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">规则名称</label>
              <TextField.Root
                value={newRule.name}
                onChange={(e) => setNewRule({ ...newRule, name: e.target.value })}
                placeholder="如：Web服务、数据库"
              />
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">协议</label>
              <Select.Root 
                value={newRule.protocol} 
                onValueChange={(value) => setNewRule({ ...newRule, protocol: value as "tcp" | "udp" })}
              >
                <Select.Trigger>
                  <div>{newRule.protocol.toUpperCase()}</div>
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="tcp">TCP</Select.Item>
                  <Select.Item value="udp">UDP</Select.Item>
                </Select.Content>
              </Select.Root>
            </Flex>

            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">来源监听</label>
              <Flex gap="2">
                <Flex direction="column" gap="1" className="flex-1">
                  <Text size="1" color="gray">IP</Text>
                  <TextField.Root
                    value={newRule.sourceIp}
                    onChange={(e) => setNewRule({ ...newRule, sourceIp: e.target.value })}
                    placeholder="any, 192.168.100.0/24"
                  />
                </Flex>
                <Flex direction="column" gap="1" className="w-20">
                  <Text size="1" color="gray">端口</Text>
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
              <label className="text-sm font-medium">目标地址</label>
              <Flex gap="2">
                <Flex direction="column" gap="1" className="flex-1">
                  <Text size="1" color="gray">IP</Text>
                  <TextField.Root
                    value={newRule.targetIp}
                    onChange={(e) => setNewRule({ ...newRule, targetIp: e.target.value })}
                    placeholder="192.168.100.10"
                  />
                </Flex>
                <Flex direction="column" gap="1" className="w-20">
                  <Text size="1" color="gray">端口</Text>
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
              <label className="text-sm font-medium">描述</label>
              <TextField.Root
                value={newRule.description}
                onChange={(e) => setNewRule({ ...newRule, description: e.target.value })}
                placeholder="转发规则说明"
              />
            </Flex>

            <Flex gap="2" justify="end">
              <Button variant="outline" onClick={() => { setIsAddingRule(false); setEditingRule(null); }}>
                取消
              </Button>
              <Button onClick={handleSaveRule}>
                保存
              </Button>
            </Flex>
          </div>
        </Card>
      )}
    </div>
  );

  return null; // 这个函数永远不会被调用，只是为了类型定义
}