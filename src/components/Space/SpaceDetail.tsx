import { useState, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { ArrowLeft, MessageSquare, MoreHorizontal, Terminal, Trash2 } from "lucide-react";
import { Button, Flex, DropdownMenu } from "@radix-ui/themes";
import { useSpaceStore } from "../../stores/spaceStore";
import { MemberCount } from "../Common/MemberCount";
import { ConfirmDialog } from "../Common/ConfirmDialog";
import { AppNavPage } from "../AppNav/AppNavPage";
import { useLayoutStore } from "../../stores/layoutStore";

export function SpaceDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { spaces, deleteSpace, loadSpaces, connectSpace } = useSpaceStore();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const { setSidebarOpen } = useLayoutStore();

  const space = spaces.find((s) => s.id === id);
  const isOwner = !!space?.owner_id;
  const callerId = space?.owner_id || "";

  // useEffect(() => {
  //   if (id && space?.status === "disconnected") {
  //     connectSpace(id);
  //   }
  // }, [id]);

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
        请选择一个空间
      </div>
    );
  }

  return (
    <div className="flex flex-col flex-1">
      {/* 顶部操作栏 */}
      <div className="h-14 flex items-center gap-3 px-4 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        <Button
          onClick={() => navigate("/")}
          variant="ghost"
          size="2"
        >
          <ArrowLeft size={20} />
        </Button>
        <div className="flex items-center gap-2">
          <div
            className={`w-2.5 h-2.5 rounded-full ${
              space.status === "connected"
                ? "bg-[var(--color-success)]"
                : space.status === "connecting"
                ? "bg-yellow-400 animate-pulse"
                : "bg-[var(--color-text-secondary)]"
            }`}
          />
          <span className="font-semibold">{space.name}</span>
          {space.virtual_ip && (
            <span className="text-xs text-[var(--color-text-secondary)] font-mono">
              {space.virtual_ip}
            </span>
          )}
          {space.status === "connected" && (
            <MemberCount spaceId={id} connected={true} />
          )}
        </div>
        <div className="flex-1" />
        <Flex gap="3" align="center">
          {/* 对话按钮 — 显式显示 */}
          <Button
            onClick={() => navigate(`/space/${id}/chat`)}
            variant="ghost"
            size="2"
          >
            <MessageSquare size={16} />
            {/* 对话 */}
          </Button>
          {/* 更多操作下拉菜单 */}
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              <Button variant="ghost" size="2">
                <MoreHorizontal size={18} />
              </Button>
            </DropdownMenu.Trigger>
            <DropdownMenu.Content>
              <DropdownMenu.Item onClick={() => navigate(`/space/${id}/logs`)}>
                <Terminal size={16} />
                日志
              </DropdownMenu.Item>
              {isOwner && (
                <DropdownMenu.Item color="red" onClick={() => setShowDeleteConfirm(true)}>
                  <Trash2 size={16} />
                  删除空间
                </DropdownMenu.Item>
              )}
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </Flex>
      </div>

      {/* 中下区域 — 应用导航页 */}
      <AppNavPage space={space} isOwner={isOwner} callerId={callerId} />

      <ConfirmDialog
        open={showDeleteConfirm}
        title="删除空间"
        message={`确定要删除空间「${space.name}」吗？此操作不可撤销。`}
        confirmText="删除"
        danger
        onConfirm={handleDelete}
        onCancel={() => setShowDeleteConfirm(false)}
      />
    </div>
  );
}