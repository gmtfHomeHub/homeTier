import { useEffect, useState } from "react";
import { Card, Text } from "@radix-ui/themes";
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
}

export function NetworkStatsPanel({ spaceId }: NetworkStatsPanelProps) {
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
    let cancelled = false;
    const loadStats = async () => {
      try {
        const [networkStats, getPeerList] = await Promise.all([
          getNetworkStats(spaceId),
          getSpacePeers(spaceId),
        ]);
        if (!cancelled) {
          setStats({
            rx_bytes: networkStats.rx_bytes,
            tx_bytes: networkStats.tx_bytes,
            avg_latency_ms: networkStats.avg_latency_ms,
          });
          setPeersList(getPeerList);
        }
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
  }, [spaceId]);

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
              {peersList.length || 0}
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

      <PeerTableDialog open={showPeersDialog} openChange={setShowPeersDialog} peerList={peersList} />
      
    </Card>
  );
}
