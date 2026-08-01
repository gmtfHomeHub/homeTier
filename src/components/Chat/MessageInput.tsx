import { useState, useRef } from "react";
import { Send, Image } from "lucide-react";
import { Button, TextArea, Flex } from "@radix-ui/themes";

interface MessageInputProps {
  spaceId: string;
  onSend: (content: string, type: string) => Promise<void>;
  disabled: boolean;
}

export function MessageInput({ spaceId, onSend, disabled }: MessageInputProps) {
  const [text, setText] = useState("");
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleSend = async () => {
    if (!text.trim() || disabled) return;
    await onSend(text.trim(), "text");
    setText("");
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleImageSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = async () => {
      const base64 = reader.result as string;
      await onSend(base64, "image");
    };
    reader.readAsDataURL(file);
  };

  return (
    <div className="p-4 border-t border-[var(--color-border)] bg-[var(--color-surface)]">
      <Flex align="center" gap="2">
        <Button
          onClick={() => fileInputRef.current?.click()}
          variant="ghost"
          size="2"
          disabled={disabled}
        >
          <Image size={24} />
        </Button>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          className="hidden"
          onChange={handleImageSelect}
        />
        <TextArea
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={disabled ? "请先连接到空间" : "输入消息，Enter 发送"}
          rows={1}
          disabled={disabled}
          className="flex-1 min-h-[var(--space-5)]"
        />
        <Button
          onClick={handleSend}
          disabled={disabled || !text.trim()}
          variant="solid"
          color="blue"
          size="2"
        >
          <Send size={18} />
        </Button>
      </Flex>
    </div>
  );
}