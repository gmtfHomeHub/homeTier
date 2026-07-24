import { useState, useEffect } from "react";
import { Button, Checkbox, Text, TextField, Select, Switch, Card, CardContent, CardHeader, CardTitle, CardDescription, ScrollArea, Flex, Table, TableBody, TableRow, TableCell, TableHead, TableHeader, AlertDialog, AlertDialogTrigger, AlertDialogContent, AlertDialogHeader, AlertDialogTitle, AlertDialogDescription, AlertDialogAction, Badge, Text as UIText } from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { useToast } from "../../hooks/useToast";
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
  type AclRule,
  type PortForwardRule
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
              <div className="mt-1 text-sm font-mono">{space.network_name}</div>
            </div>
            <div>
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">
                虚拟 IP
              </label>
              <div className="mt-1 text-sm font-mono">
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
                  <span className="w-2 h-2 rounded-full bg-current" />
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
          <AclConfig spaceId={space.id} />
        )}

        {activeTab === "forwarding" && (
          <PortForwardingConfig spaceId={space.id} />
        )}
      </div>
    </div>
  );
}

// ACL 配置组件
function AclConfig({ spaceId }: { spaceId: string }) {
  const [rules, setRules] = useState<Rule[]>([
    { id: "1", action: "allow", source: "any", dest: "192.168.100.0/24", ports: "1-65535", description: "允许本地子网访问" },
    { id: "2", action: "deny", source: "10.0.0.0/8", dest: "any", ports: "any", description: "拒绝内部网络访问" },
  ]);
  const [isAddingRule, setIsAddingRule] = useState(false);
  const [editingRule, setEditingRule] = useState<Rule | null>(null);
  const [newRule, setNewRule] = useState<Omit<Rule, "id">>({ action: "allow", source: "any", dest: "any", ports: "1-65535", description: "" });

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

  const handleEditRule = (rule: Rule) => {
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
      <div className="flex justify-between items-center">
        <h3 className="text-lg font-semibold">访问控制列表 (ACL)</h3>
        <Button onClick={() => setIsAddingRule(true)}>
          <Plus size={16} />
          添加规则
        </Button>
      </div>

      <Table variant="surface">
        <TableHeader>
          <TableRow>
            <TableHead>操作</TableHead>
            <TableHead>来源</TableHead>
            <TableHead>目标</TableHead>
            <TableHead>端口</TableHead>
            <TableHead>描述</TableHead>
            <TableHead>操作</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rules.map((rule) => (
            <TableRow key={rule.id}>
              <TableCell>
                <Badge variant={rule.action === "allow" ? "default" : "destructive"}>
                  {rule.action === "allow" ? "允许" : "拒绝"}
                </Badge>
              </TableCell>
              <TableCell className="font-mono text-sm">{rule.source}</TableCell>
              <TableCell className="font-mono text-sm">{rule.dest}</TableCell>
              <TableCell className="font-mono text-sm">{rule.ports}</TableCell>
              <TableCell className="text-sm">{rule.description}</TableCell>
              <TableCell>
                <Flex gap="2">
                  <Button size="1" variant="ghost" onClick={() => handleEditRule(rule)}>
                    <Edit2 size={14} />
                  </Button>
                  <AlertDialog>
                    <AlertDialogTrigger asChild>
                      <Button size="1" variant="ghost" color="red">
                        <Trash2 size={14} />
                      </Button>
                    </AlertDialogTrigger>
                    <AlertDialogContent>
                      <AlertDialogHeader>
                        <AlertDialogTitle>删除规则</AlertDialogTitle>
                        <AlertDialogDescription>
                          确定要删除这条 ACL 规则吗？
                        </AlertDialogDescription>
                      </AlertDialogHeader>
                      <AlertDialogAction onClick={() => handleDeleteRule(rule.id)}>
                        删除
                      </AlertDialogAction>
                    </AlertDialogContent>
                  </AlertDialog>
                </Flex>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      {(isAddingRule || editingRule) && (
        <Card>
          <CardHeader>
            <CardTitle>{editingRule ? "编辑 ACL 规则" : "添加 ACL 规则"}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <Flex direction="column" gap="2">
              <label className="text-sm font-medium">操作</label>
              <Select 
                value={newRule.action} 
                onValueChange={(value) => setNewRule({ ...newRule, action: value as "allow" | "deny" })}
              >
                <Select.Trigger>
                  <Select.Value />
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="allow">允许 (Allow)</Select.Item>
                  <Select.Item value="deny">拒绝 (Deny)</Select.Item>
                </Select.Content>
              </Select>
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
          </CardContent>
        </Card>
      )}
    </div>
  );
}

// 端口转发配置组件
function PortForwardingConfig({ spaceId }: { spaceId: string }) {
  const [rules, setRules] = useState<ForwardRule[]>([
    { id: "1", name: "Web服务", protocol: "tcp", sourceIp: "any", sourcePort: 8080, targetIp: "192.168.100.10", targetPort: 80, description: "转发到内部Web服务器" },
    { id: "2", name: "数据库", protocol: "tcp", sourceIp: "192.168.100.0/24", sourcePort: 3306, targetIp: "192.168.100.20", targetPort: 3306, description: "MySQL数据库访问" },
  ]);
  const [isAddingRule, setIsAddingRule] = useState(false);
  const [editingRule, setEditingRule] = useState<ForwardRule | null>(null);
  const [newRule, setNewRule] = useState<Omit<ForwardRule, "id">>({ 
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

  const handleEditRule = (rule: ForwardRule) => {
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
      <div className="flex justify-between items-center">
        <h3 className="text-lg font-semibold">端口转发</h3>
        <Button onClick={() => setIsAddingRule(true)}>
          <Plus size={16} />
          添加规则
        </Button>
      </div>

      <Table variant="surface">
        <TableHeader>
          <TableRow>
            <TableHead>名称</TableHead>
            <TableHead>协议</TableHead>
            <TableHead>来源</TableHead>
            <TableHead>目标</TableHead>
            <TableHead>描述</TableHead>
            <TableHead>操作</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rules.map((rule) => (
            <TableRow key={rule.id}>
              <TableCell>
                <Text size="2" weight="medium">{rule.name}</Text>
              </TableCell>
              <TableCell>
                <Badge variant="outline">{rule.protocol.toUpperCase()}</Badge>
              </TableCell>
              <TableCell>
                <Text size="1">
                  {rule.sourceIp}:{rule.sourcePort}
                </Text>
              </TableCell>
              <TableCell>
                <Text size="1">
                  {rule.targetIp}:{rule.targetPort}
                </Text>
              </TableCell>
              <TableCell className="text-sm">{rule.description}</TableCell>
              <TableCell>
                <Flex gap="2">
                  <Button size="1" variant="ghost" onClick={() => handleEditRule(rule)}>
                    <Edit2 size={14} />
                  </Button>
                  <AlertDialog>
                    <AlertDialogTrigger asChild>
                      <Button size="1" variant="ghost" color="red">
                        <Trash2 size={14} />
                      </Button>
                    </AlertDialogTrigger>
                    <AlertDialogContent>
                      <AlertDialogHeader>
                        <AlertDialogTitle>删除规则</AlertDialogTitle>
                        <AlertDialogDescription>
                          确定要删除这条端口转发规则吗？
                        </AlertDialogDescription>
                      </AlertDialogHeader>
                      <AlertDialogAction onClick={() => handleDeleteRule(rule.id)}>
                        删除
                      </AlertDialogAction>
                    </AlertDialogContent>
                  </AlertDialog>
                </Flex>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      {(isAddingRule || editingRule) && (
        <Card>
          <CardHeader>
            <CardTitle>{editingRule ? "编辑端口转发规则" : "添加端口转发规则"}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
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
              <Select 
                value={newRule.protocol} 
                onValueChange={(value) => setNewRule({ ...newRule, protocol: value as "tcp" | "udp" })}
              >
                <Select.Trigger>
                  <Select.Value />
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="tcp">TCP</Select.Item>
                  <Select.Item value="udp">UDP</Select.Item>
                </Select.Content>
              </Select>
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
                    onChange={(e) => setNewRule({ ...newRule, sourcePort: e.target.value.parse().unwrap_or(0) })}
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
                    onChange={(e) => setNewRule({ ...newRule, targetPort: e.target.value.parse().unwrap_or(0) })}
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
          </CardContent>
        </Card>
      )}
    </div>
  );

  // 类型定义
  interface Rule {
    id: string;
    action: "allow" | "deny";
    source: string;
    dest: string;
    ports: string;
    description: string;
  }

  interface ForwardRule {
    id: string;
    name: string;
    protocol: "tcp" | "udp";
    sourceIp: string;
    sourcePort: number;
    targetIp: string;
    targetPort: number;
    description: string;
  }

  return null; // 这个函数永远不会被调用，只是为了类型定义
}
      </div>
    </div>
  );
}