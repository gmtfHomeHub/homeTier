import { useEffect, useState } from "react";
import { Card, Text, Flex, Grid } from "@radix-ui/themes";
import { useTranslation } from "react-i18next";
import { Signal, Wifi, Activity, Users } from "lucide-react";
import { getSpacePeers, getNetworkStats } from "../../utils/api";
import type { PeerInfo } from "../../types";
import { PeerTableDialog } from "./peerTableDialog";

interface StatsData {
  rx_bytes: number;
  tx_bytes: number;
  avg_latency_ms: number;
}

interface NetworkStatsPanelProps {
  spaceId: string;
  connected?: boolean;
}

export function NetworkStatsPanel({ spaceId, connected = false }: NetworkStatsPanelProps) {
  const { t } = useTranslation();
  const [stats, setStats] = useState<StatsData>({
    rx_bytes: 0,
    tx_bytes: 0,
    avg_latency_ms: 0,
  });
  const [loading, setLoading] = useState(true);
  const [peersList, setPeersList] = useState<PeerInfo[]>([]);
  const [showPeersDialog, setShowPeersDialog] = useState(false);

  useEffect(() => {
    if (!connected) {
      setStats({ rx_bytes: 0, tx_bytes: 0, avg_latency_ms: 0 });
      setPeersList([]);
      setLoading(false);
      return;
    }
    let cancelled = false;
    const loadStats = async () => {
      try {
        getNetworkStats(spaceId).then((networkStats) => {
          if (!cancelled) {
            setStats({
              rx_bytes: networkStats.rx_bytes,
              tx_bytes: networkStats.tx_bytes,
              avg_latency_ms: networkStats.avg_latency_ms,
            });
          }
        });
        getSpacePeers(spaceId).then((getPeerList) => {
          if (!cancelled) {
            setPeersList(getPeerList);
          }
        });
      } catch (error) {
        console.error("Failed to load network stats:", error);
      } finally {
        setLoading(false);
      }
    };

    const interval = setInterval(loadStats, 2000);
    loadStats();

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [spaceId, connected]);

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
          <Text size="2" weight="bold">{loading ? t('network.stats') : t('network.stats')}</Text>
        </div>
        <div className="pb-4 text-center">
          <Text size="1" color="gray">{loading ? t("common.loading") : t('network.disconnected')}</Text>
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
