import { create } from "zustand";
import * as api from "../utils/api";
import type { Message } from "../types";

function tempId(): string {
  return `temp-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

interface ChatStore {
  messages: Record<string, Message[]>;
  loading: boolean;
  error: string | null;

  loadMessages: (spaceId: string, limit?: number) => Promise<void>;
  sendMessage: (spaceId: string, content: string, msgType?: string) => Promise<void>;
  addMessage: (spaceId: string, message: Message) => void;
}

export const useChatStore = create<ChatStore>((set, get) => ({
  messages: {},
  loading: false,
  error: null,

  loadMessages: async (spaceId, limit = 50) => {
    set({ loading: true, error: null });
    try {
      const msgs = await api.getMessageHistory(spaceId, limit);
      set((state) => ({
        messages: { ...state.messages, [spaceId]: msgs.reverse() },
        loading: false,
      }));
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  sendMessage: async (spaceId, content, msgType = "text") => {
    // 乐观发送：先插入临时消息
    const tid = tempId();
    const temp: Message = {
      id: tid,
      space_id: spaceId,
      sender_id: "",
      sender_name: "",
      msg_type: (msgType === "image" ? "image" : "text") as "text" | "image" | "system",
      content,
      timestamp: new Date().toISOString(),
      status: "sending",
    };
    set((state) => ({
      messages: {
        ...state.messages,
        [spaceId]: [...(state.messages[spaceId] || []), temp],
      },
    }));

    try {
      const msg = await api.sendMessage(spaceId, content, msgType);
      // 用服务端返回替换临时消息
      set((state) => {
        const existing = state.messages[spaceId] || [];
        return {
          messages: {
            ...state.messages,
            [spaceId]: existing.map((m) => (m.id === tid ? msg : m)),
          },
        };
      });
    } catch (e) {
      // 标记失败
      set((state) => {
        const existing = state.messages[spaceId] || [];
        return {
          error: String(e),
          messages: {
            ...state.messages,
            [spaceId]: existing.map((m) =>
              m.id === tid ? { ...m, status: "failed" as const } : m
            ),
          },
        };
      });
    }
  },

  addMessage: (spaceId, message) => {
    set((state) => {
      const existing = state.messages[spaceId] || [];
      if (existing.some((m) => m.id === message.id)) {
        return state;
      }
      return {
        messages: { ...state.messages, [spaceId]: [...existing, message] },
      };
    });
  },
}));
