// src/components/Space/VpnStatusBadge.tsx - VPN 状态徽章组件
import { useTranslation } from "react-i18next";
import {
  Badge,
  BadgeDot,
  Tooltip,
  Box,
  Text,
} from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { SpaceStatus } from "../../enum";

interface VpnStatusBadgeProps {
  spaceId: string;
  className?: string;
  showTooltip?: boolean;
}

export function VpnStatusBadge({ spaceId, className = "", showTooltip = true }: VpnStatusBadgeProps) {
  const { t } = useTranslation();
  const space = useSpaceStore((state) => state.spaces.find((s) => s.id === spaceId));

  if (!space) return null;

  const status = space.status;
  const virtualIp = space.virtual_ip;

  const getStatusConfig = () => {
    switch (status) {
      case SpaceStatus.ING: // connecting
        return {
          color: "amber" as const,
          label: t("vpn.status.preparing"),
          icon: "⏳",
        };
      case SpaceStatus.CED: // connected
        return {
          color: "green" as const,
          label: t("vpn.status.connected", { ip: virtualIp ?? "" }),
          icon: "🔒",
        };
      case SpaceStatus.DIS:
      default:
        return {
          color: "gray" as const,
          label: t("vpn.status.disconnected"),
          icon: "⭘",
        };
    }
  };

  const { color, label, icon } = getStatusConfig();

  const content = (
    <Badge.Root className={className}>
      <Badge.Dot color={color} />
      <Badge.Text asChild>
        <Text size="1" weight="medium">
          {icon} {label}
        </Text>
      </Badge.Text>
    </Badge.Root>
  );

  if (!showTooltip) return content;

  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        {content}
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content side="top" align="center" sideOffset={5}>
          <Box
            display="flex"
            flexDirection="column"
            gap="2"
            p="2"
            bg="var(--radix-ui-colors-gray-1)"
            borderRadius="4"
          >
            <Text weight="bold" size="2" color="gray">
              {t("vpn.vpnStatus")}
            </Text>
            <Text size="1" color="gray">
              {t("vpn.virtualIp", { ip: virtualIp ?? t("vpn.notAssigned") })}
            </Text>
            <Text size="1" color="gray">
              {t("vpn.status." + (status === SpaceStatus.ING ? "preparing" : status === SpaceStatus.CED ? "connected" : "disconnected"))}
            </Text>
          </Box>
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

export function MobileVpnIndicator({ spaceId }: { spaceId: string }) {
  const { t } = useTranslation();
  const space = useSpaceStore((state) => state.spaces.find((s) => s.id === spaceId));

  if (!space || space.status !== SpaceStatus.CED) return null;

  return (
    <Badge.Root color="green" variant="surface" className="gap-1">
      <Badge.Dot />
      <Badge.Text asChild>
        <span style={{ fontSize: "11px" }}>🔒 {t("vpn.vpnActive")}</Badge.Text>
      </Badge.Text>
    </Badge.Root>
  );
}