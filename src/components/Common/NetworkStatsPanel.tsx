import { useEffect, useState } from "react";
import { Card, Text, Flex, Grid } from "@radix-ui/themes";
import { useTranslation } from "react-i18next";
import { Signal, Wifi, Activity, Users } from "lucide-react";
import type { PeerInfo } from "../../types";
import { PeerTableDialog } from "./peerTableDialog";
import { usePeerStore } from "../../stores/peerStore";

interface NetworkStatsPanelProps {
  spaceId: string;
  connected?: boolean;
}

export function NetworkStatsPanel({ spaceId, connected = false }: NetworkStatsPanelProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(true);
  const [showPeersDialog, setShowPeersDialog] = useState(false);
  const peersList = usePeerStore((s) => s.peers[spaceId] ?? []);
  const stats = usePeerStore((s) => s.stats[spaceId] ?? { rx_bytes: 0, tx_bytes: 0, avg_latency_ms: 0 });
  const startPolling = usePeerStore((s) => s.startPolling);
  const stopPolling = usePeerStore((s) => s.stopPolling);
  const clearPeers = usePeerStore((s) => s.clearPeers);

  useEffect(() => {
    if (!connected) {
      // 断开时：清零统计、停止轮询、清理 peers 与 stats
      stopPolling(spaceId);
      clearPeers(spaceId);
      setLoading(false);
      return;
    }
    // 启动统一轮询（peers + stats）
    startPolling(spaceId);
    // 首次拉取后取消 loading
    const timer = setTimeout(() => setLoading(false), 100);
    return () => {
      clearTimeout(timer);
      stopPolling(spaceId);
    };
  }, [spaceId, connected, startPolling, stopPolling, clearPeers]);

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
        <div className="pb-4 text-center">
          <Text size="1" color="gray">{t("common.loading")}</Text>
        </div>
      </Card>
    );
  }

  const cls = `text-[var(--color-${connected ? 'success' : 'info'})]`;
  return (
    <Card className="w-full">
      <div className="pb-4 px-4 border-b border-[var(--color-border)]">
        <Text size="2" weight="bold">{t('network.stats')}</Text>
      </div>
      <div className="px-4">
        <Grid columns={{ initial: "2", sm: "4" }} gap="4">
          <Flex align="center" gap="4">
            <div className="flex items-center gap-2 text-sm">
              <Signal className={cls} />
              <span className="font-medium">{t('network.downstream')}</span>
            </div>
            <Text size="1" weight="bold" className={cls}>
              {formatLocalBytes(stats.rx_bytes)}
            </Text>
          </Flex>

          <Flex align="center" gap="4">
            <div className="flex items-center gap-2 text-sm">
              <Signal className={cls} />
              <span className="font-medium">{t('network.upstream')}</span>
            </div>
            <Text size="1" weight="bold" className={cls}>
              {formatLocalBytes(stats.tx_bytes)}
            </Text>
          </Flex>

          <Flex align="center" gap="4">
            <div className="flex items-center gap-2 text-sm">
              <Activity className={cls} />
              <span className="font-medium">{t('network.latency')}</span>
            </div>
            <Text size="1" weight="bold" className={cls}>
              {formatLatency(stats.avg_latency_ms)}
            </Text>
          </Flex>
          <Flex align="center" gap="4">
            <div
              className="flex items-center gap-2 text-sm cursor-pointer hover:bg-[var(--color-surface-hover)] rounded transition-colors"
              onClick={() => setShowPeersDialog(true)}
            >
              <Wifi className={cls} />
              <span className="font-medium">{t('network.peers')}</span>
              <Users size={12} className="text-[var(--color-text-secondary)]" />
              <Text size="1" weight="bold" className={cls}>
                {peersList.length || 0}
              </Text>
            </div>
          </Flex>

        </Grid>

        <div className="pt-4 border-t border-[var(--color-border)]">
          <div className="flex items-center justify-between text-xs text-[var(--color-text-secondary)]">
            <span>{t('network.networkActivity')}</span>
            <span>{t('network.lastUpdated')}</span>
          </div>
        </div>
      </div>

      <PeerTableDialog open={showPeersDialog} openChange={setShowPeersDialog} peerList={peersList} />
      
    </Card>
  );
}