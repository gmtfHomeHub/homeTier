import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  Button,
  Badge,
  Dialog,
  ScrollArea,
  Checkbox,
  Select,
  Text,
  Flex,
} from "@radix-ui/themes";
import { Monitor, MonitorOff, X, Share2, Square, UserPlus } from "lucide-react";
import { useScreenStore, type ScreenQuality } from "../../stores/screenStore";
import { screenService, SCREEN_QUALITY_PRESETS } from "../../services/screen";
import { listMembers } from "../../utils/api";
import type { Member } from "../../types";

const QUALITY_OPTIONS: ScreenQuality[] = ["smooth", "standard", "hd"];

export function ScreenViewer() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();

  const isSharing = useScreenStore((s) => s.isSharing);
  const sourceName = useScreenStore((s) => s.sourceName);
  const quality = useScreenStore((s) => s.quality);
  const viewerCount = useScreenStore((s) => s.viewerCount);

  const watching = useScreenStore((s) => s.watching);
  const sharerName = useScreenStore((s) => s.sharerName);
  const remoteStream = useScreenStore((s) => s.remoteStream);
  const remoteQuality = useScreenStore((s) => s.remoteQuality);
  const shareEnded = useScreenStore((s) => s.shareEnded);

  const [shareQuality, setShareQuality] = useState<ScreenQuality>("standard");
  const [inviteOpen, setInviteOpen] = useState(false);
  const [members, setMembers] = useState<Member[]>([]);
  const [selectedIps, setSelectedIps] = useState<string[]>([]);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (!id) return;
    void screenService.startWatching(id);
    return () => {
      void screenService.stopWatching();
    };
  }, [id]);

  const openInvite = async () => {
    if (!id) return;
    try {
      const data = await listMembers(id);
      setMembers(data.filter((m) => m.is_online));
      setSelectedIps([]);
    } catch {
      setMembers([]);
    }
    setInviteOpen(true);
  };

  const toggleIp = (ip: string) => {
    setSelectedIps((prev) =>
      prev.includes(ip) ? prev.filter((x) => x !== ip) : [...prev, ip],
    );
  };

  const startShareFromInvite = async () => {
    if (!id) return;
    setStarting(true);
    try {
      const ips = selectedIps.slice();
      await screenService.startShare(id, ips, shareQuality);
      setInviteOpen(false);
    } catch {
      // ignore: store.error 已经由 service 写入
    } finally {
      setStarting(false);
    }
  };

  const stopShare = () => {
    void screenService.stopShare();
  };

  return (
    <div className="relative flex-1 bg-black overflow-hidden">
      {/* 视频区域 */}
      {isSharing ? (
        <div className="w-full h-full flex flex-col items-center justify-center text-white">
          <div className="flex items-center gap-3 mb-2">
            <Share2 size={32} />
            <span className="text-lg font-medium">{t("screen.sharingActive")}</span>
          </div>
          <p className="text-sm text-white/70">
            {sourceName} · {viewerCount} {t("screen.viewers")}
          </p>
        </div>
      ) : remoteStream ? (
        <video
          ref={(el) => {
            if (el && remoteStream && el.srcObject !== remoteStream) {
              el.srcObject = remoteStream;
            }
          }}
          autoPlay
          playsInline
          className="w-full h-full object-contain"
        />
      ) : (
        <div className="w-full h-full flex flex-col items-center justify-center text-[var(--color-text-secondary)]">
          <MonitorOff size={48} className="mb-3 opacity-50" />
          <p className="text-sm">
            {shareEnded
              ? t("screen.shareEnded")
              : watching
                ? t("screen.waitingShare")
                : t("screen.notWatching")}
          </p>
        </div>
      )}

      {/* 顶部居中标题区 */}
      <div className="absolute top-3 left-1/2 -translate-x-1/2 flex items-center gap-2 pointer-events-none">
        {isSharing && (
          <div className="flex items-center gap-2 bg-black/60 backdrop-blur rounded-full px-3 py-1.5 text-white text-xs">
            <Monitor size={14} />
            <span className="truncate max-w-[160px]">{sourceName || t("screen.sharing")}</span>
            <Badge size="1" color="green" variant="soft">
              {viewerCount} {t("screen.viewers")}
            </Badge>
          </div>
        )}
        {watching && sharerName && (
          <div className="flex items-center gap-2 bg-black/60 backdrop-blur rounded-full px-3 py-1.5 text-white text-xs">
            <Monitor size={14} />
            <span className="truncate max-w-[160px]">{sharerName}</span>
          </div>
        )}
        {remoteStream && (
          <Badge size="1" color="blue" variant="soft">
            {t(SCREEN_QUALITY_PRESETS[(remoteQuality || "standard") as ScreenQuality].labelKey)}
          </Badge>
        )}
      </div>

      {/* 右上角操作区 */}
      <div className="absolute top-3 right-3 flex items-center gap-2">
        {!isSharing && !remoteStream && (
          <Button onClick={openInvite} variant="solid" color="iris" size="1">
            <Share2 size={14} />
            {t("screen.startShare")}
          </Button>
        )}
        {isSharing && (
          <>
            <Select.Root
              size="1"
              value={shareQuality}
              onValueChange={(v) => setShareQuality(v as ScreenQuality)}
            >
              <Select.Trigger />
              <Select.Content>
                {QUALITY_OPTIONS.map((q) => (
                  <Select.Item key={q} value={q}>
                    {t(SCREEN_QUALITY_PRESETS[q].labelKey)}
                  </Select.Item>
                ))}
              </Select.Content>
            </Select.Root>
            <Button onClick={stopShare} variant="solid" color="red" size="1">
              <Square size={14} />
              {t("screen.stopShare")}
            </Button>
          </>
        )}
        <Button
          onClick={() => id && navigate(`/space/${id}`)}
          variant="solid"
          color="gray"
          size="1"
          className="bg-black/60"
        >
          <X size={14} />
          {t("screen.exitView")}
        </Button>
      </div>

      {/* 邀请成员对话框 */}
      <Dialog.Root open={inviteOpen} onOpenChange={setInviteOpen}>
        <Dialog.Content style={{ maxWidth: 480, width: "calc(100vw - 48px)" }}>
          <Dialog.Title>
            <Flex align="center" gap="2">
              <UserPlus size={18} />
              {t("screen.inviteTitle")}
            </Flex>
          </Dialog.Title>
          <Dialog.Description size="2" mb="3">
            {t("screen.inviteDesc")}
          </Dialog.Description>

          <Flex align="center" justify="between" mb="3">
            <Text size="2" weight="medium">
              {t("screen.quality")}
            </Text>
            <Select.Root
              size="2"
              value={shareQuality}
              onValueChange={(v) => setShareQuality(v as ScreenQuality)}
            >
              <Select.Trigger />
              <Select.Content>
                {QUALITY_OPTIONS.map((q) => (
                  <Select.Item key={q} value={q}>
                    {t(SCREEN_QUALITY_PRESETS[q].labelKey)}
                  </Select.Item>
                  ))}
              </Select.Content>
            </Select.Root>
          </Flex>

          <ScrollArea type="auto" scrollbars="vertical" style={{ maxHeight: 260 }}>
            {members.length === 0 ? (
              <Text size="2" color="gray">
                {t("screen.noOnlineMembers")}
              </Text>
            ) : (
              <Flex direction="column" gap="2">
                {members.map((m) => (
                  <Flex
                    key={m.id}
                    align="center"
                    justify="between"
                    p="2"
                    gap="3"
                    className="rounded-lg border border-[var(--color-border)]"
                  >
                    <label className="flex-1 cursor-pointer">
                      <Flex align="center" gap="3">
                        <Checkbox
                          checked={selectedIps.includes(m.virtual_ip ?? "")}
                          onCheckedChange={() => toggleIp(m.virtual_ip ?? "")}
                        />
                        <Flex direction="column">
                          <Text size="2" weight="medium">
                            {m.nickname}
                          </Text>
                          {m.virtual_ip && (
                            <Text size="1" color="gray" className="font-mono">
                              {m.virtual_ip}
                            </Text>
                          )}
                        </Flex>
                      </Flex>
                    </label>
                  </Flex>
                ))}
              </Flex>
            )}
          </ScrollArea>

          <Flex gap="3" mt="4" justify="end">
            <Dialog.Close>
              <Button variant="soft" color="gray">
                {t("common.cancel")}
              </Button>
            </Dialog.Close>
            <Button
              onClick={startShareFromInvite}
              disabled={starting}
              color="iris"
            >
              {starting ? t("common.starting") : t("screen.startShare")}
            </Button>
          </Flex>
        </Dialog.Content>
      </Dialog.Root>
    </div>
  );
}
