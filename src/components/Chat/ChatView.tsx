import { useEffect, useState } from "react";
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
import { View } from "../Common/PageView";

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
        {t("space.selectSpace")}
      </div>
    );
  }

  return (
    <View
      header={
        <>
          <Button
            onClick={() => navigate(`/space/${id}`)}
            variant="ghost"
            size="2"
          >
            <ArrowLeft size={20} />
          </Button>
          <span className="font-semibold">{t("chat.title")}</span>
          <div className="flex-1" />
        </>
      }
    >
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
    </View>
  );
}
