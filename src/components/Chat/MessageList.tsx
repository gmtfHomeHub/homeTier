import type { Message } from "../../types";
import { formatTimestamp } from "../../utils/format";
import { useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Check, CheckCheck, Loader2, XCircle } from "lucide-react";

interface MessageListProps {
  messages: Message[];
}

function StatusIndicator({ status }: { status: Message["status"] }) {
  const { t } = useTranslation();
  if (status === "sending") {
    return (
      <span className="inline-flex items-center gap-1 text-xs text-yellow-500">
        <Loader2 size={12} className="animate-spin" />
        {t("chat.statusSending")}
      </span>
    );
  }
  if (status === "failed") {
    return (
      <span className="inline-flex items-center gap-1 text-xs text-red-500">
        <XCircle size={12} />
        {t("chat.statusFailed")}
      </span>
    );
  }
  if (status === "delivered") {
    return (
      <span className="inline-flex items-center gap-1 text-xs text-[var(--color-success)]">
        <CheckCheck size={12} />
        {t("chat.statusDelivered")}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
      <Check size={12} />
      {t("chat.statusSent")}
    </span>
  );
}

export function MessageList({ messages }: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  if (messages.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--color-text-secondary)] text-sm">
        暂无消息，开始聊天吧
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-2">
      {messages.map((msg) => (
        <div
          key={msg.id}
          className={`flex ${msg.msg_type === "system" ? "justify-center" : "flex-col"}`}
        >
          {msg.msg_type === "system" ? (
            <div className="text-xs text-[var(--color-text-secondary)] bg-[var(--color-border)]/50 px-3 py-1 rounded-full">
              {msg.content}
            </div>
          ) : (
            <div className="group">
              <div className="flex items-baseline gap-2">
                <span className="text-sm font-medium text-[var(--color-primary)]">
                  {msg.sender_name}
                </span>
                <span className="text-xs text-[var(--color-text-secondary)] opacity-0 group-hover:opacity-100 transition-opacity">
                  {formatTimestamp(msg.timestamp)}
                </span>
                <StatusIndicator status={msg.status} />
              </div>
              {msg.msg_type === "image" ? (
                <img
                  src={msg.content}
                  alt="图片"
                  className="max-w-sm rounded-lg mt-1 cursor-pointer hover:opacity-90"
                  onClick={() => window.open(msg.content)}
                />
              ) : (
                <p className="text-sm mt-0.5">{msg.content}</p>
              )}
            </div>
          )}
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );
}
