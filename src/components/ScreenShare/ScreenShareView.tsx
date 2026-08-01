import { useState, useEffect, useCallback } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Button, Dialog, Checkbox, Flex, Text } from "@radix-ui/themes";
import { Monitor, MonitorOff, Eye, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useScreenStore, type ScreenQuality } from "../../stores/screenStore";
import { screenService, SCREEN_QUALITY_PRESETS } from "../../services/screen";
import { listMembers } from "../../utils/api";

export function ScreenShareView() {
  const { id: spaceId } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();

  const isSharing = useScreenStore((s) => s.isSharing);
  const sourceName = useScreenStore((s) => s.sourceName);
  const quality = useScreenStore((s) => s.quality);
  const viewerCount = useScreenStore((s) => s.viewerCount);
  const error = useScreenStore((s) => s.error);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [members, setMembers] = useState<
    { id: string; nickname: string; virtual_ip?: string; is_online: boolean }[]
  >([]);
  const [selectedIps, setSelectedIps] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [starting, setStarting] = useState(false);

  const openDialog = useCallback(async () => {
    setDialogOpen(true);
    setSelectedIps([]);
    if (!spaceId) return;
    try {
      const list = await listMembers(spaceId);
      setMembers(list.filter((m) => m.virtual_ip));
    } catch {
      setMembers([]);
    }
  }, [spaceId]);

  const toggleIp = (ip: string) => {
    setSelectedIps((prev) => (prev.includes(ip) ? prev.filter((x) => x !== ip) : [...prev, ip]));
  };

  const handleStart = async () => {
    if (!spaceId) return;
    setStarting(true);
    try {
      await screenService.startShare(spaceId, selectedIps, quality);
      setDialogOpen(false);
    } catch (e) {
      console.error("Screen share start failed:", e);
    } finally {
      setStarting(false);
    }
  };

  const handleStop = async () => {
    await screenService.stopShare();
  };

  const handleQuality = async (q: ScreenQuality) => {
    await screenService.setQuality(q);
  };

  const onlineMembers = members.filter((m) => m.is_online);

  return (
    <div className="bg-[var(--color-surface)] rounded-xl p-4 border border-[var(--color-border)]">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <Monitor size={20} className="text-[var(--color-primary)]" />
          <span className="font-medium text-sm">{t("screen.title")}</span>
        </div>
        {isSharing ? (
          <Button
            onClick={handleStop}
            variant="solid"
            color="red"
            size="2"
          >
            <MonitorOff size={14} />
            {t("screen.stop")}
          </Button>
        ) : (
          <Button
            onClick={openDialog}
            variant="soft"
            color="blue"
            size="2"
          >
            <Monitor size={14} />
            {t("screen.start")}
          </Button>
        )}
      </div>

      {error && (
        <div className="mb-2 text-xs text-red-500">{error}</div>
      )}

      {isSharing && (
        <div className="space-y-3">
          <div className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
            <Monitor size={12} />
            <span className="truncate">{sourceName}</span>
          </div>

          {/* 画质选择 */}
          <div>
            <div className="text-xs text-[var(--color-text-secondary)] mb-1">
              {t("screen.quality")}
            </div>
            <Flex gap="2">
              {(Object.keys(SCREEN_QUALITY_PRESETS) as ScreenQuality[]).map((q) => (
                <Button
                  key={q}
                  size="1"
                  variant={quality === q ? "solid" : "soft"}
                  color={quality === q ? "blue" : "gray"}
                  onClick={() => handleQuality(q)}
                >
                  {t(SCREEN_QUALITY_PRESETS[q].labelKey)}
                </Button>
              ))}
            </Flex>
          </div>

          {/* 查看者数量 */}
          <div className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
            <Eye size={12} />
            <span>
              {viewerCount} {t("screen.viewerCount")}
            </span>
          </div>
        </div>
      )}

      {/* 进入查看页 */}
      <div className="mt-3 border-t border-[var(--color-border)] pt-3">
        <Button
          variant="ghost"
          size="2"
          className="w-full"
          onClick={() => spaceId && navigate(`/space/${spaceId}/screen`)}
        >
          <Eye size={14} />
          {t("screen.view")}
        </Button>
      </div>

      {/* 邀请成员弹窗 */}
      <Dialog.Root open={dialogOpen} onOpenChange={setDialogOpen}>
        <Dialog.Content className="w-[420px]">
          <Dialog.Title size="3">{t("screen.inviteTitle")}</Dialog.Title>
          <Dialog.Description size="2" className="text-[var(--color-text-secondary)]">
            {t("screen.inviteDesc")}
          </Dialog.Description>

          <div className="my-3 max-h-56 overflow-y-auto">
            {onlineMembers.length === 0 ? (
              <Text size="2" className="text-[var(--color-text-secondary)]">
                {t("screen.noOnlineMembers")}
              </Text>
            ) : (
              <Flex direction="column" gap="2">
                {onlineMembers.map((m) => (
                  <label key={m.id} className="flex items-center gap-2 cursor-pointer">
                    <Checkbox
                      checked={selectedIps.includes(m.virtual_ip!)}
                      onCheckedChange={() => toggleIp(m.virtual_ip!)}
                    />
                    <Text size="2">{m.nickname}</Text>
                  </label>
                ))}
              </Flex>
            )}
            <Text size="1" className="mt-2 block text-[var(--color-text-secondary)]">
              {t("screen.inviteHint")}
            </Text>
          </div>

          <Flex gap="2" justify="end">
            <Button variant="soft" color="gray" onClick={() => setDialogOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="solid"
              color="blue"
              disabled={starting}
              onClick={handleStart}
            >
              <Users size={14} />
              {starting ? t("common.loading") : t("screen.start")}
            </Button>
          </Flex>
        </Dialog.Content>
      </Dialog.Root>
    </div>
  );
}
