import { useEffect, useState } from "react";
import { Card, Text } from "@radix-ui/themes";
import { useTranslation } from "react-i18next";
import { Signal, Wifi, Activity } from "lucide-react";

interface NetworkStats {
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
  const [stats, setStats] = useState<NetworkStats>({
    rx_bytes: 0,
    tx_bytes: 0,
    avg_latency_ms: 0,
    connected_peers: 0,
  });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const loadStats = async () => {
      try {
        const response = await fetch(`/api/space/${spaceId}/status`);
        const data = await response.json();
        if (data) {
          setStats({
            rx_bytes: data.rx_bytes || 0,
            tx_bytes: data.tx_bytes || 0,
            avg_latency_ms: data.avg_latency_ms || 0,
            connected_peers: data.connected_peers || 0,
          });
        }
      } catch (error) {
        console.error("Failed to load network stats:", error);
      } finally {
        setLoading(false);
      }
    };

    const interval = setInterval(loadStats, 5000); // 每5秒更新一次
    loadStats(); // 立即加载一次

    return () => clearInterval(interval);
  }, [spaceId]);

  const formatBytes = (bytes: number): string => {
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
          <Text size="1" color="gray">加载中...</Text>
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
          {/* 接收流量 */}
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-sm">
              <Signal className="text-[var(--color-info)]" />
              <span className="font-medium">{t('network.downstream')}</span>
            </div>
            <Text size="1" weight="bold" className="text-[var(--color-info)]">
              {formatBytes(stats.rx_bytes)}
            </Text>
          </div>

          {/* 发送流量 */}
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-sm">
              <Signal className="text-[var(--color-info)]" />
              <span className="font-medium">{t('network.upstream')}</span>
            </div>
            <Text size="1" weight="bold" className="text-[var(--color-info)]">
              {formatBytes(stats.tx_bytes)}
            </Text>
          </div>

          {/* 平均延迟 */}
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-sm">
              <Activity className="text-[var(--color-success)]" />
              <span className="font-medium">{t('network.latency')}</span>
            </div>
            <Text size="1" weight="bold" className="text-[var(--color-success)]">
              {formatLatency(stats.avg_latency_ms)}
            </Text>
          </div>

          {/* 连接节点数 */}
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-sm">
              <Wifi className="text-[var(--color-success)]" />
              <span className="font-medium">{t('network.peers')}</span>
            </div>
            <Text size="1" weight="bold" className="text-[var(--color-success)]">
              {stats.connected_peers}
            </Text>
          </div>
        </div>

        {/* 流量趋势指示器 */}
        <div className="mt-4 pt-4 border-t border-[var(--color-border)]">
          <div className="flex items-center justify-between text-xs text-[var(--color-text-secondary)]">
            <span>{t('network.networkActivity')}</span>
            <span>{t('network.lastUpdated')}</span>
          </div>
        </div>
      </div>
    </Card>
  );
}