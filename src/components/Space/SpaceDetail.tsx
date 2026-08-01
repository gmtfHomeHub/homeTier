import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  Users,
  ArrowLeft,
  MessageSquare,
  MoreHorizontal,
  Terminal,
  Trash2,
} from "lucide-react";
import { Button, Flex, DropdownMenu } from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useSpaceConnect } from "../../hooks/useSpaceConnect";
import { MemberManager } from "./MemberManager";
import { ConfirmDialog } from "../Common/ConfirmDialog";
import { ScreenShareView } from "../ScreenShare/ScreenShareView";
import { AppNavPage } from "../AppNav/AppNavPage";
import { NetworkStatsPanel } from "../Common/NetworkStatsPanel";
import { SpaceStatus } from "../../enum";
import { View } from "../Common/PageView";

export function SpaceDetail() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { spaces, deleteSpace, loadSpaces } = useSpaceStore();
  const { logEnabled } = useSettingsStore();
  const { disconnectingId, connect, disconnect } = useSpaceConnect();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [showMemberManager, setShowMemberManager] = useState(false);

  const space = spaces.find((s) => s.id === id);
  const isOwner = !!space?.owner_id;
  const callerId = space?.owner_id || "";
  const isRunning = space?.status === SpaceStatus.CED;

  const handleDelete = async () => {
    if (!id || !space) return;
    setDeleting(true);
    try {
      await deleteSpace(id, callerId);
      await loadSpaces();
      navigate("/");
    } catch (e) {
      alert(String(e));
    } finally {
      setDeleting(false);
      setShowDeleteConfirm(false);
    }
  };

  if (!id || !space) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)]">
        {t("space.selectSpace")}
      </div>
    );
  }

  return (
    <View
      header={
        <>
          <Button onClick={() => navigate("/")} variant="ghost" size="2">
            <ArrowLeft size={20} />
          </Button>
          <Flex gap="2" align="center">
            <div
              className={`w-2.5 h-2.5 rounded-full ${
                isRunning
                  ? "bg-[var(--color-success)]"
                  : space.status === "connecting"
                    ? "bg-yellow-400 animate-pulse"
                    : "bg-[var(--color-text-secondary)]"
              }`}
            />
            <span className="font-semibold">{space.name}</span>
            {!isRunning ? (
              <Button onClick={() => connect(space.id)} variant="soft" size="1">
                {t("space.connect")}
              </Button>
            ) : (
              <Button
                onClick={() => disconnect(space.id)}
                disabled={disconnectingId === id}
                loading={disconnectingId === id}
                variant="outline"
                size="1"
              >
                {disconnectingId === id
                  ? t("space.disconnecting")
                  : t("space.disconnect")}
              </Button>
            )}
            {space.virtual_ip && (
              <span className="text-xs text-[var(--color-text-secondary)] font-mono">
                {space.virtual_ip}
              </span>
            )}
          </Flex>
          <div className="flex-1" />
          <Flex gap="3" align="center">
            <Button
              onClick={() => navigate(`/space/${id}/chat`)}
              variant="ghost"
              size="2"
            >
              <MessageSquare size={16} />
            </Button>
            <DropdownMenu.Root>
              <DropdownMenu.Trigger>
                <Button variant="ghost" size="2">
                  <MoreHorizontal size={18} />
                </Button>
              </DropdownMenu.Trigger>
              <DropdownMenu.Content>
                {logEnabled && (
                  <DropdownMenu.Item
                    onClick={() => navigate(`/space/${id}/logs`)}
                  >
                    <Terminal size={16} />
                    {t("space.logs")}
                  </DropdownMenu.Item>
                )}
                {isOwner && (
                  <DropdownMenu.Item onClick={() => setShowMemberManager(true)}>
                    <Users size={16} />
                    {t("space.memberManager")}
                  </DropdownMenu.Item>
                )}
                {isOwner && (
                  <DropdownMenu.Item
                    color="red"
                    onClick={() => setShowDeleteConfirm(true)}
                  >
                    <Trash2 size={16} />
                    {t("space.deleteSpace")}
                  </DropdownMenu.Item>
                )}
              </DropdownMenu.Content>
            </DropdownMenu.Root>
          </Flex>
        </>
      }
    >
      <div className="px-4 py-2">
        <NetworkStatsPanel spaceId={id} connected={isRunning} />
      </div>

      {isRunning && (
        <div className="px-4 py-2">
          <ScreenShareView />
        </div>
      )}

      <AppNavPage space={space} isOwner={isOwner} callerId={callerId} />

      {showMemberManager && (
        <MemberManager
          spaceId={id}
          callerId={callerId}
          onClose={() => setShowMemberManager(false)}
        />
      )}

      <ConfirmDialog
        open={showDeleteConfirm}
        title={t("space.deleteSpace")}
        message={t("space.confirmDeleteMessage", { name: space.name })}
        confirmText={t("space.delete")}
        danger
        onConfirm={handleDelete}
        onCancel={() => setShowDeleteConfirm(false)}
      />
    </View>
  );
}
