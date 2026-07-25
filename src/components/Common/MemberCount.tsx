import { useState, useEffect } from "react";
import { Users, X } from "lucide-react";
import { getSpacePeers } from "../../utils/api";
import { formatBytes } from "../../utils/format";
import { Button, Badge, Dialog, ScrollArea, Text } from "@radix-ui/themes";
import type { PeerInfo } from "../../types";

interface MemberCountProps {
  spaceId: string;
  connected: boolean;
}

export function MemberCount({ spaceId, connected }: MemberCountProps) {
  const [peersList, setPeersList] = useState<PeerInfo[]>([]);
  const [showDialog, setShowDialog] = useState(false);

  // 连接时每 2 秒轮询 peer 列表
  useEffect(() => {
    if (!connected) {
      setPeersList([]);
      return;
    }
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
    const timer = setInterval(poll, 2000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [spaceId, connected]);

  if (!connected) return null;

  return (
    <>
      <Button
        onClick={() => setShowDialog(true)}
        variant="ghost"
        size="1"
        className="inline-flex items-center gap-1"
        title="查看在线成员"
      >
        <Users size={12} />
        <span>{peersList.length} 个成员</span>
      </Button>

      {showDialog && (
        <Dialog.Root open={showDialog} onOpenChange={setShowDialog}>
          <Dialog.Content className="w-[820px] max-h-[80vh]">
            <div className="flex items-center justify-between mb-4">
              <Text size="2" weight="bold">在线成员</Text>
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
                      <th className="text-left p-3">主机名</th>
                      <th className="text-left p-3">虚拟 IP</th>
                      <th className="text-right p-3">延迟</th>
                      <th className="text-right p-3">丢包</th>
                      <th className="text-right p-3">接收</th>
                      <th className="text-right p-3">发送</th>
                      <th className="text-left p-3">隧道</th>
                      <th className="text-left p-3">NAT</th>
                      <th className="text-left p-3">版本</th>
                    </tr>
                  </thead>
                  <tbody>
                    {peersList.length === 0 ? (
                      <tr>
                        <td colSpan={9} className="text-center p-8 text-[var(--color-text-secondary)]">
                          暂无数据
                        </td>
                      </tr>
                    ) : (
                      peersList.map((peer, i) => (
                        <tr key={i} className="border-b">
                          <td className="p-3">
                            <span className="font-medium">{peer.hostname?.replace(/^PublicServer_/, '') || `Peer #${peer.peer_id}`}</span>
                            {peer.is_local && <Badge color="blue" ml="1" size="1">本机</Badge>}
                            {peer.hostname?.startsWith('PublicServer_') && <Badge color="gray" ml="1" size="1">服务器</Badge>}
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
              共 {peersList.length} 个在线成员
            </div>
          </Dialog.Content>
        </Dialog.Root>
      )}
    </>
  );
}