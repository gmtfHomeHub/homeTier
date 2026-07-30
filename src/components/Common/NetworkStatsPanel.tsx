import { useEffect, useState } from "react";
import { Card, Text, Dialog, ScrollArea, Badge, Button } from "@radix-ui/themes";
import { useTranslation } from "react-i18next";
import { Signal, Wifi, Activity, Users, X } from "lucide-react";
import { getSpacePeers, getNetworkStatus, getNetworkStats } from "../../utils/api";
import { formatBytes } from "../../utils/format";
import type { PeerInfo } from "../../types";

interface StatsData {
  rx_bytes: number;
  tx_bytes: number;
  avg_latency_ms: number;
  connected_peers: number;
}

interface NetworkStatsPanelProps {
  spaceId: string;
}

export function NetworkStatsPanel({ spaceId }: NetworkStatsPanelProps) {
  const { t } = useTranslation();
  const [stats, setStats] = useState<StatsData>({
    rx_bytes: 0,
    tx_bytes: 0,
    avg_latency_ms: 0,
    connected_peers: 0,
  });
  const [loading, setLoading] = useState(true);
  const [peersList, setPeersList] = useState<PeerInfo[]>([]);
  const [showPeersDialog, setShowPeersDialog] = useState(false);

  useEffect(() => {
    const loadStats = async () => {
      try {
        const [status, networkStats] = await Promise.all([
          getNetworkStatus(spaceId),
          getNetworkStats(spaceId),
        ]);
        setStats({
          rx_bytes: networkStats.rx_bytes,
          tx_bytes: networkStats.tx_bytes,
          avg_latency_ms: networkStats.avg_latency_ms,
          connected_peers: status.connected_peers,
        });
      } catch (error) {
        console.error("Failed to load network stats:", error);
      } finally {
        setLoading(false);
      }
    };

    const interval = setInterval(loadStats, 5000);
    loadStats();

    return () => clearInterval(interval);
  }, [spaceId]);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const data = await getSpacePeers(spaceId);
        if (!cancelled) setPeersList(data);
      } catch (err) {
        console.log(err);
      }
    };
    poll();
    const timer = setInterval(poll, 5000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [spaceId, stats.connected_peers]);

  const formatLocalBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  };

  const formatLatency = (latency: number): string => {
    if (latency < 1) return "< 1ms";
    return `${latency.toFixed(1)}ms`;
  };

  if (loading) {
    return (
      <Card className="w-full">
        <div className="p-4 border-b border-[var(--color-border)]">
          <Text size="2" weight="bold">{t('network.stats')}</Text>
        </div>
        <div className="p-4">
          <Text size="1" color="gray">{t("common.loading")}</Text>
        </div>
      </Card>
    );
  }

  return (
    <Card className="w-full">
      <div className="p-4 border-b border-[var(--color-border)]">
        <Text size="2" weight="bold">{t('network.stats')}</Text>
      </div>
      <div className="p-4">
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-sm">
              <Signal className="text-[var(--color-info)]" />
              <span className="font-medium">{t('network.downstream')}</span>
            </div>
            <Text size="1" weight="bold" className="text-[var(--color-info)]">
              {formatLocalBytes(stats.rx_bytes)}
            </Text>
          </div>

          <div className="space-y-1">
            <div className="flex items-center gap-2 text-sm">
              <Signal className="text-[var(--color-info)]" />
              <span className="font-medium">{t('network.upstream')}</span>
            </div>
            <Text size="1" weight="bold" className="text-[var(--color-info)]">
              {formatLocalBytes(stats.tx_bytes)}
            </Text>
          </div>

          <div className="space-y-1">
            <div className="flex items-center gap-2 text-sm">
              <Activity className="text-[var(--color-success)]" />
              <span className="font-medium">{t('network.latency')}</span>
            </div>
            <Text size="1" weight="bold" className="text-[var(--color-success)]">
              {formatLatency(stats.avg_latency_ms)}
            </Text>
          </div>

          <div
            className="space-y-1 cursor-pointer hover:bg-[var(--color-surface-hover)] rounded p-1 -m-1 transition-colors"
            onClick={() => setShowPeersDialog(true)}
            role="button"
            tabIndex={0}
          >
            <div className="flex items-center gap-2 text-sm">
              <Wifi className="text-[var(--color-success)]" />
              <span className="font-medium">{t('network.peers')}</span>
              <Users size={12} className="text-[var(--color-text-secondary)]" />
            </div>
            <Text size="1" weight="bold" className="text-[var(--color-success)]">
              {stats.connected_peers}
            </Text>
          </div>
        </div>

        <div className="mt-4 pt-4 border-t border-[var(--color-border)]">
          <div className="flex items-center justify-between text-xs text-[var(--color-text-secondary)]">
            <span>{t('network.networkActivity')}</span>
            <span>{t('network.lastUpdated')}</span>
          </div>
        </div>
      </div>

      <Dialog.Root open={showPeersDialog} onOpenChange={setShowPeersDialog}>
        <Dialog.Content className="w-[820px] max-h-[80vh]">
          <div className="flex items-center justify-between mb-4">
            <Text size="2" weight="bold">{t("space.onlineMembers")}</Text>
            <Dialog.Close>
              <Button variant="ghost" size="2">
                <X size={20} />
              </Button>
            </Dialog.Close>
          </div>
          <ScrollArea style={{ maxHeight: "60vh" }}>
            <div className="border rounded-lg">
              <table className="w-full">
                <thead>
                  <tr className="border-b">
                    <th className="text-left p-3">{t("space.hostname")}</th>
                    <th className="text-left p-3">{t("space.virtualIp")}</th>
                    <th className="text-right p-3">{t("network.latency")}</th>
                    <th className="text-right p-3">{t("space.packetLoss")}</th>
                    <th className="text-right p-3">{t("network.downstream")}</th>
                    <th className="text-right p-3">{t("network.upstream")}</th>
                    <th className="text-left p-3">{t("space.tunnel")}</th>
                    <th className="text-left p-3">{t("space.nat")}</th>
                    <th className="text-left p-3">{t("space.version")}</th>
                  </tr>
                </thead>
                <tbody>
                  {peersList.length === 0 ? (
                    <tr>
                      <td colSpan={9} className="text-center p-8 text-[var(--color-text-secondary)]">
                        {t("space.noData")}
                      </td>
                    </tr>
                  ) : (
                    peersList.map((peer, i) => (
                      <tr key={i} className="border-b">
                        <td className="p-3">
                          <span className="font-medium">{peer.hostname?.replace(/^PublicServer_/, '') || `Peer #${peer.peer_id}`}</span>
                          {peer.is_local && <Badge color="blue" ml="1" size="1">{t("space.local")}</Badge>}
                          {peer.hostname?.startsWith('PublicServer_') && <Badge color="gray" ml="1" size="1">{t("space.server")}</Badge>}
                        </td>
                        <td className="p-3 font-mono text-[var(--color-text-secondary)]">{peer.virtual_ip || '-'}</td>
                        <td className="p-3 font-mono text-right">{peer.latency_ms != null ? `${peer.latency_ms.toFixed(1)}ms` : '-'}</td>
                        <td className="p-3 font-mono text-right">{peer.loss_rate != null ? `${peer.loss_rate.toFixed(1)}%` : '-'}</td>
                        <td className="p-3 font-mono text-right">{peer.rx_bytes != null ? formatBytes(peer.rx_bytes) : '-'}</td>
                        <td className="p-3 font-mono text-right">{peer.tx_bytes != null ? formatBytes(peer.tx_bytes) : '-'}</td>
                        <td className="p-3 font-mono text-[var(--color-text-secondary)]">{peer.tunnel_proto || '-'}</td>
                        <td className="p-3 font-mono text-[var(--color-text-secondary)]">{peer.nat_type || '-'}</td>
                        <td className="p-3 font-mono text-[var(--color-text-secondary)]">{peer.version || '-'}</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </ScrollArea>
          <div className="mt-4 pt-3 border-t border-[var(--color-border)] text-xs text-[var(--color-text-secondary)]">
            {t("space.totalOnlineMembers", { count: peersList.length })}
          </div>
        </Dialog.Content>
      </Dialog.Root>
    </Card>
  );
}
