import { useState, useEffect } from "react";
import { Users, X } from "lucide-react";
import { getSpacePeers } from "../../utils/api";
import { formatBytes } from "../../utils/format";
import { Button, Badge, Table, Dialog, ScrollArea } from "@radix-ui/themes";
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
              <Dialog.Title className="m-0 text-lg font-semibold">在线成员</Dialog.Title>
              <Dialog.Close>
                <Button variant="ghost" size="2">
                  <X size={20} />
                </Button>
              </Dialog.Close>
            </div>
            <ScrollArea style={{ maxHeight: "60vh" }}>
              <Table.Root>
                <Table.Header>
                  <Table.Row>
                    <Table.ColumnHeaderCell>主机名</Table.ColumnHeaderCell>
                    <Table.ColumnHeaderCell>虚拟 IP</Table.ColumnHeaderCell>
                    <Table.ColumnHeaderCell align="right">延迟</Table.ColumnHeaderCell>
                    <Table.ColumnHeaderCell align="right">丢包</Table.ColumnHeaderCell>
                    <Table.ColumnHeaderCell align="right">接收</Table.ColumnHeaderCell>
                    <Table.ColumnHeaderCell align="right">发送</Table.ColumnHeaderCell>
                    <Table.ColumnHeaderCell>隧道</Table.ColumnHeaderCell>
                    <Table.ColumnHeaderCell>NAT</Table.ColumnHeaderCell>
                    <Table.ColumnHeaderCell>版本</Table.ColumnHeaderCell>
                  </Table.Row>
                </Table.Header>
                <Table.Body>
                  {peersList.length === 0 ? (
                    <Table.Row>
                      <Table.Cell colSpan={9} align="center" className="h-24 text-[var(--color-text-secondary)]">
                        暂无数据
                      </Table.Cell>
                    </Table.Row>
                  ) : (
                    peersList.map((peer, i) => (
                      <Table.Row key={i}>
                        <Table.Cell>
                          <span className="font-medium">{peer.hostname?.replace(/^PublicServer_/, '') || `Peer #${peer.peer_id}`}</span>
                          {peer.is_local && <Badge color="blue" ml="1" size="1">本机</Badge>}
                          {peer.hostname?.startsWith('PublicServer_') && <Badge color="gray" ml="1" size="1">服务器</Badge>}
                        </Table.Cell>
                        <Table.Cell className="font-mono text-[var(--color-text-secondary)]">{peer.virtual_ip || '-'}</Table.Cell>
                        <Table.Cell align="right" className="font-mono">{peer.latency_ms != null ? `${peer.latency_ms.toFixed(1)}ms` : '-'}</Table.Cell>
                        <Table.Cell align="right" className="font-mono">{peer.loss_rate != null ? `${peer.loss_rate.toFixed(1)}%` : '-'}</Table.Cell>
                        <Table.Cell align="right" className="font-mono">{peer.rx_bytes != null ? formatBytes(peer.rx_bytes) : '-'}</Table.Cell>
                        <Table.Cell align="right" className="font-mono">{peer.tx_bytes != null ? formatBytes(peer.tx_bytes) : '-'}</Table.Cell>
                        <Table.Cell className="font-mono text-[var(--color-text-secondary)]">{peer.tunnel_proto || '-'}</Table.Cell>
                        <Table.Cell className="font-mono text-[var(--color-text-secondary)]">{peer.nat_type || '-'}</Table.Cell>
                        <Table.Cell className="font-mono text-[var(--color-text-secondary)]">{peer.version || '-'}</Table.Cell>
                      </Table.Row>
                    ))
                  )}
                </Table.Body>
              </Table.Root>
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