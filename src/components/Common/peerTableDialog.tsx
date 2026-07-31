import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { formatBytes } from "../../utils/format";
import { Button, Badge, Dialog, ScrollArea, Text } from "@radix-ui/themes";
import type { PeerInfo } from "../../types";

interface PeerTableProps {
  peerList: PeerInfo[];
  open: boolean;
  openChange: (flag: boolean) => void;
}

export function PeerTableDialog({ peerList, open, openChange }: PeerTableProps) {
  const { t } = useTranslation();
  return open ? (
        <Dialog.Root open={open} onOpenChange={openChange}>
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
                      <th className="p-3 text-left">{t("space.hostname")}</th>
                      <th className="p-3 text-left">{t("space.virtualIp")}</th>
                      <th className="p-3 text-right">{t("network.latency")}</th>
                      <th className="p-3 text-right">{t("space.packetLoss")}</th>
                      <th className="p-3 text-right">{t("network.downstream")}</th>
                      <th className="p-3 text-right">{t("network.upstream")}</th>
                      <th className="p-3 text-left">{t("space.tunnel")}</th>
                      <th className="p-3 text-left">{t("space.nat")}</th>
                      <th className="p-3 text-left">{t("space.version")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {peerList.length === 0 ? (
                      <tr>
                        <td colSpan={9} className="text-center p-8 text-[var(--color-text-secondary)]">
                          {t("space.noData")}
                        </td>
                      </tr>
                    ) : (
                      peerList.map((peer, i) => (
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
              {t("space.totalOnlineMembers", { count: peerList.length })}
            </div>
          </Dialog.Content>
        </Dialog.Root>
  ) : null;
}
