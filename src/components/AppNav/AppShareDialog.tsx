import { useState, useEffect, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Check, X, Share2, Wifi, Users, Copy, HelpCircle } from "lucide-react";
import { Button, TextField, Flex, Text, Badge, ScrollArea, Box } from "@radix-ui/themes";
import Tip from "../Common/Tip";
import { toastSuccess, toastError } from "../../utils/toast";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { QRCodeSVG } from "qrcode.react";
import { generateAddAppLink } from "../../utils/api";
import { usePeerStore } from "../../stores/peerStore";
import type { Space, SpaceApp, PeerInfo } from "../../types";

interface AppShareDialogProps {
  space: Space;
  onClose: () => void;
}

function filterPeers(peers: PeerInfo[]): PeerInfo[] {
  return peers.filter(
    (p) => !p.is_local && !p.hostname?.startsWith("PublicServer_")
  );
}

function peerDisplayName(peer: PeerInfo): string {
  const name = peer.hostname?.replace(/^PublicServer_/, "") ?? `Peer #${peer.peer_id}`;
  const ip = peer.virtual_ip ? ` ${peer.virtual_ip}` : "";
  return `${name}${ip}`;
}

export function AppShareDialog({ space, onClose }: AppShareDialogProps) {
  const { t } = useTranslation();
  const [apps, setApps] = useState<SpaceApp[]>([]);
  const [loadingApps, setLoadingApps] = useState(true);
  const [selectedApps, setSelectedApps] = useState<Set<string>>(new Set());
  const [selectedPeers, setSelectedPeers] = useState<Set<number>>(new Set());
  const [link, setLink] = useState<string>("");
  const [generating, setGenerating] = useState(false);
  const [copied, setCopied] = useState(false);

  // 从 peerStore 获取已过滤的节点列表
  const allPeers = usePeerStore((s) => s.peers[space.id] ?? []);
  const filteredPeers = useMemo(() => filterPeers(allPeers), [allPeers]);

  // 按分组组织应用（复用 AppNavPage 逻辑）
  const appGroups = useMemo(() => {
    const grouped = apps.reduce<Record<string, SpaceApp[]>>((acc, app) => {
      const cat = app.category || t("appNav.uncategorized");
      if (!acc[cat]) acc[cat] = [];
      acc[cat].push(app);
      return acc;
    }, {});
    return Object.keys(grouped).sort().map((cat) => ({ title: cat, apps: grouped[cat] }));
  }, [apps, t]);

  // 加载应用列表
  useEffect(() => {
    const load = async () => {
      try {
        const { listApps } = await import("../../utils/api");
        const data = await listApps(space.id);
        setApps(data);
      } catch (e) {
        toastError(String(e));
      } finally {
        setLoadingApps(false);
      }
    };
    load();
  }, [space.id]);

  // 选择/取消选择应用
  const toggleApp = useCallback((appId: string) => {
    setSelectedApps((prev) => {
      const next = new Set(prev);
      if (next.has(appId)) next.delete(appId);
      else next.add(appId);
      return next;
    });
  }, []);

  // 选择/取消选择节点
  const togglePeer = useCallback((peerId: number) => {
    setSelectedPeers((prev) => {
      const next = new Set(prev);
      if (next.has(peerId)) next.delete(peerId);
      else next.add(peerId);
      return next;
    });
  }, []);

  const handleGenerate = async () => {
    if (selectedApps.size === 0 || selectedPeers.size === 0) return;
    setGenerating(true);
    try {
      const newLink = await generateAddAppLink(
        space.id,
        Array.from(selectedApps),
        Array.from(selectedPeers)
      );
      setLink(newLink);
      setCopied(false);
    } catch (e) {
      toastError(String(e));
    } finally {
      setGenerating(false);
    }
  };

  const handleCopy = async () => {
    try {
      await writeText(link);
      setCopied(true);
      toastSuccess(t("space.copiedToClipboard"));
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      toastError(String(e));
    }
  };

  const canGenerate = selectedApps.size > 0 && selectedPeers.size > 0;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[var(--color-surface)] rounded-xl p-6 w-full max-w-[calc(100vw-24px)] sm:w-[900px] max-h-[85vh] flex flex-col shadow-xl animate-fade-in">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">{t("space.appShareTitle")}</h2>
          <Button onClick={onClose} variant="ghost" size="2">
            <X size={20} />
          </Button>
        </div>

        {loadingApps ? (
          <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)]">
            {t("common.loading")}
          </div>
        ) : link ? (
          // 显示生成的二维码和链接
          <div className="flex-1 flex flex-col items-center overflow-y-auto space-y-4">
            <div className="p-3 bg-white rounded-xl w-full max-w-[320px]">
              <div className="w-full aspect-square">
                <QRCodeSVG
                  value={link}
                  size={260}
                  level="M"
                  marginSize={4}
                  className="w-full h-full"
                />
              </div>
            </div>

            <Flex align="center" gap="3" className="w-full max-w-[400px]">
              <TextField.Root
                value={link}
                readOnly
                className="flex-1 font-mono text-xs"
              />
              <Button onClick={handleCopy} variant="ghost" size="2">
                {copied ? <Check size={18} /> : <Copy size={18} />}
              </Button>
            </Flex>

            <Flex className="w-full justify-end gap-2 pt-2">
              <Button onClick={() => setLink("")} variant="outline" size="2">
                {t("common.cancel")}
              </Button>
              <Button onClick={onClose} variant="solid" color="blue" size="2">
                {t("common.done")}
              </Button>
            </Flex>
          </div>
        ) : (
          // 双栏选择界面
          <div className="flex-1 flex overflow-hidden space-y-4">
            {/* 左栏：应用列表 */}
            <div className="flex-1 min-w-0 flex flex-col bg-[var(--color-surface-hover)] rounded-xl p-4">
              <div className="flex items-center justify-between mb-3">
                <Text size="3" weight="bold">{t("space.selectApps")}</Text>
                <Badge color="blue" variant="soft">
                  {selectedApps.size} / {apps.length}
                </Badge>
              </div>
              <ScrollArea className="flex-1">
                <div className="space-y-3">
                  {appGroups.map((group) => (
                    <Box key={group.title} className="space-y-2">
                      <Text size="2" weight="bold" className="text-[var(--color-text-secondary)]">
                        {group.title}
                      </Text>
                      {group.apps.map((app) => (
                        <label
                          key={app.id}
                          className="flex items-center gap-3 p-2 rounded-lg hover:bg-[var(--color-surface)] cursor-pointer transition-colors"
                        >
                          <input
                            type="checkbox"
                            checked={selectedApps.has(app.id)}
                            onChange={() => toggleApp(app.id)}
                            className="accent-blue"
                          />
                          <div className="flex-1 min-w-0">
                            <Text size="2" weight="medium" className="truncate">
                              {app.name}
                            </Text>
                            <Text size="1" color="gray" className="truncate">
                              {app.protocol}//{app.hostname}:{app.port}{app.pathname}
                            </Text>
                          </div>
                        </label>
                      ))}
                    </Box>
                  ))}
                </div>
              </ScrollArea>
            </div>

            {/* 右栏：节点列表 */}
            <div className="flex-1 min-w-0 flex flex-col bg-[var(--color-surface-hover)] rounded-xl p-4">
              <div className="flex items-center justify-between mb-3">
                <Flex align="center" gap="2">
                  <Text size="3" weight="bold">{t("space.selectNodes")}</Text>
                  <Badge color="green" variant="soft">
                    {selectedPeers.size} / {filteredPeers.length}
                  </Badge>
                </Flex>
                <Tip
                  content={
                    <Text size="2">
                      {t("space.nodeFilterHint")}
                    </Text>
                  }
                >
                  <HelpCircle size={14} className="text-[var(--color-text-secondary)] cursor-pointer" />
                </Tip>
              </div>
              <ScrollArea className="flex-1">
                {filteredPeers.length === 0 ? (
                  <div className="flex flex-col items-center justify-center h-full text-[var(--color-text-secondary)]">
                    <Wifi size={32} className="mb-2 opacity-50" />
                    <Text>{t("space.noAvailableNodes")}</Text>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {filteredPeers.map((peer) => (
                      <label
                        key={peer.peer_id}
                        className="flex items-center gap-3 p-2 rounded-lg hover:bg-[var(--color-surface)] cursor-pointer transition-colors"
                      >
                        <input
                          type="checkbox"
                          checked={selectedPeers.has(peer.peer_id)}
                          onChange={() => togglePeer(peer.peer_id)}
                          className="accent-blue"
                        />
                        <div className="flex-1 min-w-0">
                          <Flex align="center" gap="2" className="mb-1">
                            <Text size="2" weight="medium" className="truncate">
                              {peer.hostname?.replace(/^PublicServer_/, "") ?? `Peer #${peer.peer_id}`}
                            </Text>
                            {peer.is_local && (
                              <Badge color="blue" size="1">{t("space.local")}</Badge>
                            )}
                          </Flex>
                          <Text size="1" color="gray" className="truncate">
                            {peer.virtual_ip || t("space.noVirtualIp")}
                          </Text>
                        </div>
                      </label>
                    ))}
                  </div>
                )}
              </ScrollArea>
            </div>
          </div>
        )}

        {/* 底部确认按钮（仅选择态显示） */}
        {!link && (
          <Flex justify="end" gap="2" className="pt-4 border-t border-[var(--color-border)]">
            <Button onClick={onClose} variant="outline" size="2">
              {t("common.cancel")}
            </Button>
            <Button
              onClick={handleGenerate}
              disabled={!canGenerate || generating}
              loading={generating}
              variant="solid"
              color="blue"
              size="2"
            >
              {generating ? t("common.loading") : t("space.confirmShare")}
            </Button>
          </Flex>
        )}
      </div>
    </div>
  );
}