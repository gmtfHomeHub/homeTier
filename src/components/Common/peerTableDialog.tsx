import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { formatBytes } from "../../utils/format";
import { Button, Badge, Dialog, ScrollArea, Text } from "@radix-ui/themes";
import type { PeerInfo } from "../../types";
import { BaseTable, type ColumnDefs } from "../Common/Table";

interface PeerTableProps {
  peerList: PeerInfo[];
  open: boolean;
  openChange: (flag: boolean) => void;
}

export function PeerTableDialog({
  peerList,
  open,
  openChange,
}: PeerTableProps) {
  const { t } = useTranslation();
  const columns:ColumnDefs<PeerInfo>[] = [
      {
        title: t("space.hostname"),
        align: "left",
        field: "hostname",
        render: (text, peer) => (
          <>
            <span className="font-medium">
              {text?.replace(/^PublicServer_/, "") ||
                `Peer #${peer.peer_id}`}
            </span>
            {peer.is_local && (
              <Badge color="blue" ml="1" size="1">
                {t("space.local")}
              </Badge>
            )}
            {text?.startsWith("PublicServer_") && (
              <Badge color="gray" ml="1" size="1">
                {t("space.server")}
              </Badge>
            )}
          </>
        ),
      },
      {
        title: t("space.virtualIp"),
        field: "virtual_ip",
      },
      {
        title: t("network.latency"),
        field: "latency_ms",
        align: "right",
        render: (text) => text ? `${Number(text).toFixed(1)}ms` : '-'
      },
      {
        title: t("space.packetLoss"),
        field: "loss_rate",
        align: "right",
        render: (text) => text ? `${Number(text).toFixed(1)}%` : '-'
      },
      {
        title: t("network.downstream"),
        field: "rx_bytes",
        align: "right",
        render: (text) => text ? formatBytes(text) : '-'
      },
      {
        title: t("network.upstream"),
        field: "tx_bytes",
        align: "right",
        render: (text) => text ? formatBytes(text) : '-'
      },
      {
        title: t("space.tunnel"),
        field: "tunnel_proto",
      },
      {
        title: t("space.nat"),
        field: "nat_type",
      },
      {
        title: t("space.version"),
        field: "version",
        minWidth: "130px"
      },
    ];
  return open ? (
    <Dialog.Root open={open} onOpenChange={openChange}>
      <Dialog.Content className="w-full max-w-[calc(100vw-24px)] sm:w-[820px] max-h-[80vh]">
        <div className="flex items-center justify-between mb-4">
          <Text size="2" weight="bold">
            {t("space.onlineMembers")}
          </Text>
          <Dialog.Close>
            <Button variant="ghost" size="2">
              <X size={20} />
            </Button>
          </Dialog.Close>
        </div>
        <ScrollArea style={{ maxHeight: "60vh" }}>
          <BaseTable columns={columns} dataSource={peerList}>
            <tr>
              <td
                colSpan={9}
                className="text-center p-8 text-[var(--color-text-secondary)]"
              >
                {t("space.noData")}
              </td>
            </tr>
          </BaseTable>
        </ScrollArea>
        <div className="mt-4 pt-3 border-t border-[var(--color-border)] text-xs text-[var(--color-text-secondary)]">
          {t("space.totalOnlineMembers", { count: peerList.length })}
        </div>
      </Dialog.Content>
    </Dialog.Root>
  ) : null;
}
