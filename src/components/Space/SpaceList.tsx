import { useSpaceStore } from "../../stores/spaceStore";
import { useNavigate } from "react-router-dom";
import { Share2, Trash2, Settings, X, Users } from "lucide-react";
import { useState, useEffect } from "react";
import { ConfirmDialog } from "../Common/ConfirmDialog";
import { EasyTierConfigEditor } from "../Network/EasyTierConfigEditor";
import { MemberCount } from "../Common/MemberCount";
import { Button, Flex } from "@radix-ui/themes";
import { getSystemConfig, updateSpaceConfig, getRelayPrefix } from "../../utils/api";
import type { EasyTierConfig } from "../../types/config";

export function SpaceList() {
  const { spaces, connectSpace, disconnectSpace, deleteSpace, loadSpaces } = useSpaceStore();
  const navigate = useNavigate();
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string; ownerId?: string } | null>(null);
  const [configTarget, setConfigTarget] = useState<string | null>(null);
  const [spaceConfig, setSpaceConfig] = useState<Partial<EasyTierConfig>>({});
  const [savingConfig, setSavingConfig] = useState(false);
  const [connectingId, setConnectingId] = useState<string | null>(null);

  const configSpace = spaces.find(s => s.id === configTarget);

  useEffect(() => {
    if (configTarget && configSpace) {
      // 1. 解析空间级配置
      let spaceCfg: Partial<EasyTierConfig> = {};
      if (configSpace.config_json) {
        try { spaceCfg = JSON.parse(configSpace.config_json); } catch {}
      }

      // 2. 加载系统级配置作为默认值，然后用空间级配置覆盖
      Promise.all([
        getSystemConfig(),
        getRelayPrefix(),
      ]).then(([sysJson, prefix]) => {
        localStorage.setItem("relayPrefix", prefix);
        let merged: Partial<EasyTierConfig> = {};

        // 系统级配置作为基础
        if (sysJson) {
          try { merged = JSON.parse(sysJson); } catch {}
        }

        // 空间级配置覆盖系统级（优先级：空间 > 系统）
        merged = { ...merged, ...spaceCfg };
        // 深度合并 network_identity
        if (spaceCfg.network_identity || !merged.network_identity?.network_name) {
          merged = {
            ...merged,
            network_identity: {
              network_name: merged.network_identity?.network_name || "",
              network_secret: merged.network_identity?.network_secret,
              ...spaceCfg.network_identity,
            },
          };
        }

        // 3. 自动填充 network_identity 默认值
        if (!merged.network_identity?.network_name) {
          merged.network_identity = {
            ...merged.network_identity,
            network_name: `${prefix}${configSpace.name}`,
          };
        }
        if (!merged.network_identity?.network_secret) {
          merged.network_identity = {
            ...merged.network_identity,
            network_secret: configSpace.network_secret,
          };
        }

        setSpaceConfig(merged);
      });
    }
  }, [configTarget]);

  const handleSaveConfig = async () => {
    if (!configTarget) return;
    setSavingConfig(true);
    try {
      await updateSpaceConfig(configTarget, JSON.stringify(spaceConfig));
      await loadSpaces();
      setConfigTarget(null);
    } catch (e) {
      alert(String(e));
    } finally {
      setSavingConfig(false);
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteSpace(deleteTarget.id, deleteTarget.ownerId || "");
      await loadSpaces();
      setDeleteTarget(null);
    } catch (e) {
      alert(String(e));
    }
  };

  const handleConnect = async (spaceId: string) => {
    setConnectingId(spaceId);
    try {
      await connectSpace(spaceId);
    } catch (e) {
      alert(String(e));
    } finally {
      setConnectingId(null);
    }
  };

  return (
    <div className="flex-1 p-6 overflow-y-auto">
      <h1 className="mb-6 text-2xl font-bold">空间概览</h1>

      {spaces.length === 0 && (
        <div className="text-center py-20 text-[var(--color-text-secondary)]">
          <div className="mb-4 text-5xl">🏠</div>
          <p className="mb-2 text-lg">还没有加入任何空间</p>
          <p className="text-sm">在左侧创建或加入一个空间开始使用</p>
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
        {spaces.map((space) => (
          <div
            key={space.id}
            className="bg-[var(--color-surface)] rounded-xl p-5 border border-[var(--color-border)] hover:shadow-md transition-shadow"
          >
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-3">
                <div
                  className={`w-3 h-3 rounded-full ${
                    space.status === "connected"
                      ? "bg-[var(--color-success)]"
                      : space.status === "connecting"
                      ? "bg-yellow-400 animate-pulse"
                      : "bg-[var(--color-text-secondary)]"
                  }`}
                />
                <h3 className="font-semibold truncate">{space.name}</h3>
              </div>
              <div className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
                <MemberCount spaceId={space.id} connected={space.status === "connected"} />
              </div>
            </div>
            <Flex gap="3" align="center">
              {space.status === "connected" ? (
                <>
                  <Button
                    onClick={() => navigate(`/space/${space.id}`)}
                    variant="soft"
                    size="2"
                    className="flex-1"
                  >
                    打开
                  </Button>
                  <Button
                    onClick={() => disconnectSpace(space.id)}
                    variant="outline"
                    size="2"
                  >
                    断开
                  </Button>
                </>
              ) : (
                <Button
                  onClick={() => handleConnect(space.id)}
                  disabled={space.status === "connecting" || connectingId === space.id}
                  variant="soft"
                  size="2"
                  className="flex-1"
                  loading={space.status === "connecting" || connectingId === space.id}
                >
                  {space.status === "connecting" ? "连接中..." : "连接"}
                </Button>
              )}
              <Button
                onClick={() => navigate(`/space/${space.id}`)}
                variant="ghost"
                size="2"
                title="分享"
              >
                <Share2 size={16} />
              </Button>
              <Button
                onClick={() => setConfigTarget(space.id)}
                variant="ghost"
                size="2"
                title="空间配置"
              >
                <Settings size={16} />
              </Button>
              {space.owner_id && (
              <Button
                onClick={() => setDeleteTarget({ id: space.id, name: space.name, ownerId: space.owner_id })}
                variant="ghost"
                color="red"
                size="2"
                title="删除空间"
              >
                <Trash2 size={16} />
              </Button>
            )}
            </Flex>
          </div>
        ))}
      </div>

      <ConfirmDialog
        open={deleteTarget !== null}
        title="删除空间"
        message={`确定要删除空间「${deleteTarget?.name || ""}」吗？此操作不可撤销。`}
        confirmText="删除"
        danger
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />

      {/* 空间级 EasyTier 配置弹窗 */}
      {configTarget && configSpace && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-[var(--color-surface)] rounded-xl w-[640px] max-h-[80vh] flex flex-col shadow-xl">
            <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--color-border)]">
              <h2 className="text-lg font-semibold">空间配置 - {configSpace.name}</h2>
              <Button onClick={() => setConfigTarget(null)} variant="ghost" size="2">
                <X size={20} />
              </Button>
            </div>
            <div className="flex-1 p-6 overflow-y-auto">
              <EasyTierConfigEditor
                value={spaceConfig}
                onChange={setSpaceConfig}
                title="空间级 EasyTier 配置（优先级高于系统级）"
                showNetworkIdentity={true}
              />
            </div>
            <Flex justify="end" gap="2" px="6" py="4" className="border-t border-[var(--color-border)]">
              <Button onClick={() => setConfigTarget(null)} variant="outline" size="2">
                取消
              </Button>
              <Button onClick={handleSaveConfig} disabled={savingConfig} variant="solid" color="blue" size="2" loading={savingConfig}>
                {savingConfig ? "保存中..." : "保存配置"}
              </Button>
            </Flex>
          </div>
        </div>
      )}

      {/* 在线成员列表弹窗 */}
    </div>
  );
}