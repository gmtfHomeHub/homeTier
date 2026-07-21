import { create } from "zustand";
import * as api from "../utils/api";
import type { Message } from "../types";

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
    try {
      const msg = await api.sendMessage(spaceId, content, msgType);
      set((state) => {
        const existing = state.messages[spaceId] || [];
        return {
          messages: { ...state.messages, [spaceId]: [...existing, msg] },
        };
      });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  addMessage: (spaceId, message) => {
    set((state) => {
      const existing = state.messages[spaceId] || [];
      return {
        messages: { ...state.messages, [spaceId]: [...existing, message] },
      };
    });
  },
}));