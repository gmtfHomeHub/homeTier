import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { useChatStore } from "../../stores/chatStore";
import { useSpaceStore } from "../../stores/spaceStore";
import { setActiveChatSpace } from "../../services/realtime";
import { MessageList } from "./MessageList";
import { MessageInput } from "./MessageInput";
import { ArrowLeft, Mic, Monitor, FileUp, MoreHorizontal } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Button, DropdownMenu } from "@radix-ui/themes";

export function ChatView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { messages, loadMessages } = useChatStore();
  const { spaces } = useSpaceStore();
  const [sending, setSending] = useState(false);
  const { t } = useTranslation();

  const space = spaces.find((s) => s.id === id);
  const spaceMessages = id ? messages[id] || [] : [];

  useEffect(() => {
    if (id) {
      loadMessages(id);
    }
  }, [id]);

  useEffect(() => {
    setActiveChatSpace(id || null);
    return () => setActiveChatSpace(null);
  }, [id]);

  if (!id || !space) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)]">
        {t('space.selectSpace')}
      </div>
    );
  }

  return (
    <div className="flex flex-col flex-1">
      {/* 头部 — 对话专用操作栏 */}
      <div className="h-14 flex items-center gap-3 px-4 border-b border-[var(--color-border)] bg-[var(--color-surface)]">
        <Button
          onClick={() => navigate(`/space/${id}`)}
          variant="ghost"
          size="2"
        >
          <ArrowLeft size={20} />
        </Button>
        <span className="font-semibold">{t('chat.title')}</span>
        <div className="flex-1" />
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            <Button variant="ghost" size="2">
              <MoreHorizontal size={18} />
            </Button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Content>
            <DropdownMenu.Item onClick={() => navigate(`/space/${id}/voice`)}>
              <Mic size={16} />
              实时语音
            </DropdownMenu.Item>
            <DropdownMenu.Item onClick={() => navigate(`/space/${id}/files`)}>
              <FileUp size={16} />
              文件共享
            </DropdownMenu.Item>
            <DropdownMenu.Item onClick={() => navigate(`/space/${id}/screen`)}>
              <Monitor size={16} />
              屏幕共享
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>

      {/* 消息列表 */}
      <MessageList messages={spaceMessages} />

      {/* 输入框 */}
      <MessageInput
        spaceId={id}
        onSend={async (content, type) => {
          setSending(true);
          try {
            const { sendMessage } = useChatStore.getState();
            await sendMessage(id, content, type);
          } finally {
            setSending(false);
          }
        }}
        disabled={sending || space.status !== "connected"}
      />

      {/* 在线成员列表弹窗 */}
    </div>
  );
}
