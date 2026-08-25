// src/components/Space/VpnStatusBadge.tsx - VPN 状态徽章组件
import { useTranslation } from "react-i18next";
import { Badge, Box, Text } from "@radix-ui/themes";
import Tip from "../Common/Tip";
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
    <Badge color={color} variant="soft" className={className}>
      <Text size="1" weight="medium">
        {icon} {label}
      </Text>
    </Badge>
  );

  if (!showTooltip) return content;

  return (
    <Tip
      content={
        <Box
          display="block"
          style={{
            background: "var(--radix-ui-colors-gray-1)",
            borderRadius: "4px",
            padding: "8px",
          }}
        >
          <Text weight="bold" size="2" color="gray">
            {t("vpn.vpnStatus")}
          </Text>
          <Text size="1" color="gray" as="div">
            {t("vpn.virtualIp", { ip: virtualIp ?? t("vpn.notAssigned") })}
          </Text>
          <Text size="1" color="gray" as="div">
            {t(
              "vpn.status." +
                (status === SpaceStatus.ING
                  ? "preparing"
                  : status === SpaceStatus.CED
                    ? "connected"
                    : "disconnected")
            )}
          </Text>
        </Box>
      }
    >
      {content}
    </Tip>
  );
}

export function MobileVpnIndicator({ spaceId }: { spaceId: string }) {
  const { t } = useTranslation();
  const space = useSpaceStore((state) => state.spaces.find((s) => s.id === spaceId));

  if (!space || space.status !== SpaceStatus.CED) return null;

  return (
    <Badge color="green" variant="surface" className="gap-1">
      <Text size="1">🔒 {t("vpn.vpnActive")}</Text>
    </Badge>
  );
}