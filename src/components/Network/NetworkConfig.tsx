import { useState } from "react";
import { Button, Checkbox, Text } from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { Settings, Globe, Shield, Sliders } from "lucide-react";

export function NetworkConfig() {
  const { spaces, currentSpaceId } = useSpaceStore();
  const space = spaces.find((s) => s.id === currentSpaceId);
  const [activeTab, setActiveTab] = useState<"basic" | "advanced" | "acl">("basic");

  if (!space) {
    return (
      <div className="text-center py-8 text-[var(--color-text-secondary)] text-sm">
        请先选择一个空间
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
          基本
        </Button>
        <Button
          onClick={() => setActiveTab("advanced")}
          variant={activeTab === "advanced" ? "solid" : "ghost"}
          color="blue"
          size="2"
          className="flex-1"
        >
          <Sliders size={16} />
          高级
        </Button>
        <Button
          onClick={() => setActiveTab("acl")}
          variant={activeTab === "acl" ? "solid" : "ghost"}
          color="blue"
          size="2"
          className="flex-1"
        >
          <Shield size={16} />
          ACL
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
          <div className="text-sm text-[var(--color-text-secondary)]">
            <p>ACL 规则配置（开发中）</p>
          </div>
        )}
      </div>
    </div>
  );
}