import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { useChatStore } from "../stores/chatStore";
import type { Message } from "../types";

/**
 * 后端 emit 的 new_message 事件载荷 = 原始 ChatMessage（Rust 序列化）
 * 包含 signature 字段，无 status 字段。
 */
export interface RawChatMessage {
  id: string;
  space_id: string;
  sender_id: string;
  sender_name: string;
  msg_type: string;
  content: string;
  timestamp: string;
  signature?: string;
}

type SignalHandler = (spaceId: string, payload: string, raw: RawChatMessage) => void;

const signalHandlers: Set<SignalHandler> = new Set();

/** 当前正在查看的聊天空间（用于抑制通知） */
let activeChatSpace: string | null = null;
let notifPermission: boolean | null = null;

/** 注册信令处理器（Voice/Screen 的 WebRTC Hub 使用） */
export function registerSignalHandler(handler: SignalHandler): () => void {
  signalHandlers.add(handler);
  return () => signalHandlers.delete(handler);
}

/** 标记当前正在查看的聊天空间（ChatView 挂载时调用，null 表示离开） */
export function setActiveChatSpace(spaceId: string | null) {
  activeChatSpace = spaceId;
}

async function ensureNotificationPermission(): Promise<boolean> {
  if (notifPermission !== null) return notifPermission;
  try {
    if (isTauriEnv()) {
      let granted = await isPermissionGranted();
      if (!granted) {
        granted = (await requestPermission()) === "granted";
      }
      notifPermission = granted;
    } else if (typeof Notification !== "undefined") {
      if (Notification.permission === "default") {
        notifPermission = (await Notification.requestPermission()) === "granted";
      } else {
        notifPermission = Notification.permission === "granted";
      }
    } else {
      notifPermission = false;
    }
    return notifPermission;
  } catch (e) {
    console.warn("[realtime] notification permission error:", e);
    notifPermission = false;
    return false;
  }
}

async function maybeNotify(raw: RawChatMessage) {
  // 正在查看该空间且窗口聚焦时不打扰
  if (activeChatSpace === raw.space_id && document.hasFocus()) return;

  const granted = await ensureNotificationPermission();
  if (!granted) return;

  const preview = raw.content.length > 60 ? `${raw.content.slice(0, 60)}…` : raw.content;
  if (isTauriEnv()) {
    sendNotification({ title: raw.sender_name, body: preview });
  } else {
    new Notification(raw.sender_name, { body: preview });
  }
}

function toMessage(raw: RawChatMessage): Message {
  const msgType = raw.msg_type === "image" || raw.msg_type === "system" ? raw.msg_type : "text";
  return {
    id: raw.id,
    space_id: raw.space_id,
    sender_id: raw.sender_id,
    sender_name: raw.sender_name,
    msg_type: msgType,
    content: raw.content,
    timestamp: raw.timestamp,
    status: "sent",
  };
}

/**
 * 初始化实时事件中枢，统一监听 new_message 并分流：
 * - signal → 信令处理器
 * - 其他 → 入库 + 系统通知
 * 返回取消监听函数。应用启动时调用一次。
 *
 * 双模式：
 * - Tauri 桌面：listen("new_message")
 * - Web（服务器模式）：WebSocket /api/cmd/ws/events 订阅全局事件流，
 *   MessageSent 事件的 payload.message 即 RawChatMessage
 */
export async function initRealtime(): Promise<UnlistenFn> {
  const isTauri = isTauriEnv();

  if (!isTauri) {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const ws = new WebSocket(`${protocol}//${window.location.host}/api/cmd/ws/events`);
    const onOpen = new Promise<void>((resolve) => {
      ws.onopen = () => resolve();
    });
    await onOpen;

    ws.onmessage = (event) => {
      try {
        const envelope = JSON.parse(event.data);
        if (!envelope || envelope.type !== "Event" || !envelope.event) return;
        if (envelope.event.event_type !== "MessageSent") return;
        const raw = envelope.event.payload?.message as RawChatMessage;
        dispatchRaw(raw);
      } catch (e) {
        console.error("[realtime] ws event parse error:", e);
      }
    };

    return () => {
      ws.close();
    };
  }

  const unlisten = await listen<RawChatMessage>("new_message", (event) => {
    dispatchRaw(event.payload);
  });

  return unlisten;
}

function isTauriEnv(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function dispatchRaw(raw: RawChatMessage) {
  if (!raw || !raw.id) return;

  if (raw.msg_type === "signal") {
    signalHandlers.forEach((handler) => {
      try {
        handler(raw.space_id, raw.content, raw);
      } catch (e) {
        console.error("[realtime] signal handler error:", e);
      }
    });
    return;
  }

  const message = toMessage(raw);
  if (message.msg_type === "text" || message.msg_type === "image") {
    useChatStore.getState().addMessage(raw.space_id, message);
    void maybeNotify(raw);
  }
}
