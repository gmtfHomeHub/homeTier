import { registerSignalHandler as registerRawSignalHandler, type RawChatMessage } from "./realtime";
import { useSpaceStore } from "../stores/spaceStore";
import { sendSignal as sendSignalApi } from "../utils/api";

/**
 * WebRTC 信令中枢
 *
 * 基于 chat 消息通道（msg_type="signal"）的轻量信令协议：
 * 信封以 JSON 存于 ChatMessage.content，from 由发送方 virtual_ip 填充，
 * 接收方通过 realtime.ts 收到后在此解析、自过滤、定向过滤并按 kind 路由。
 *
 * 供 Voice / Screen 模块注册使用：
 *   const off = registerSignalHandler("voice", (spaceId, env, fromMember) => {...});
 */

export type SignalKind = "voice" | "screen" | "file";

export interface SignalEnvelope {
  kind: SignalKind;
  type: string; // offer / answer / ice / join / leave / viewers / ...
  from: string; // 发送方 virtual_ip
  to?: string; // 目标 virtual_ip（缺省 = 广播）
  data?: unknown;
}

type SignalKindHandler = (
  spaceId: string,
  env: SignalEnvelope,
  fromMember: { id: string; nickname: string; virtualIp?: string } | null
) => void;

const kindHandlers = new Map<SignalKind, Set<SignalKindHandler>>();

let wired = false;

/** 注册某类信令（voice/screen）的处理器，返回取消函数 */
export function registerSignalHandler(
  kind: SignalKind,
  handler: SignalKindHandler
): () => void {
  if (!kindHandlers.has(kind)) {
    kindHandlers.set(kind, new Set());
  }
  kindHandlers.get(kind)!.add(handler);
  ensureWired();
  return () => {
    kindHandlers.get(kind)?.delete(handler);
  };
}

/** 获取当前空间的自身虚拟 IP（用于信封 from / 自过滤） */
export function getSelfVirtualIp(spaceId: string): string | undefined {
  return useSpaceStore.getState().spaces.find((s) => s.id === spaceId)?.virtual_ip;
}

let memberCache: Record<string, { id: string; nickname: string; virtualIp?: string }[]> = {};

/** 预取空间成员（供 from 解析昵称），可手动调用或由 Voice/Screen 初始化 */
export async function preloadMembers(spaceId: string): Promise<void> {
  try {
    const { listMembers } = await import("../utils/api");
    const members = await listMembers(spaceId);
    memberCache[spaceId] = members.map((m) => ({
      id: m.id,
      nickname: m.nickname,
      virtualIp: m.virtual_ip,
    }));
  } catch (e) {
    console.warn("[signal] preloadMembers failed:", e);
  }
}

export function resolveMember(spaceId: string, ip: string) {
  const members = memberCache[spaceId] || [];
  const hit = members.find((m) => m.virtualIp === ip);
  if (hit) return hit;
  // 兜底：ip 即昵称不可读时给个占位
  return { id: ip, nickname: ip, virtualIp: ip };
}

function ensureWired() {
  if (wired) return;
  wired = true;
  registerRawSignalHandler((spaceId, payload, raw) => {
    if (!raw) return;
    let env: SignalEnvelope;
    try {
      env = JSON.parse(payload) as SignalEnvelope;
    } catch {
      return; // 非信令信封，忽略
    }
    if (!env || typeof env.kind !== "string") return;

    const selfIp = getSelfVirtualIp(spaceId);
    // 自过滤：忽略自己发出的信令
    if (selfIp && env.from === selfIp) return;
    // 定向过滤：to 存在且不是自己时忽略
    if (env.to && selfIp && env.to !== selfIp) return;

    const handlers = kindHandlers.get(env.kind as SignalKind);
    if (!handlers) return;
    const fromMember = resolveMember(spaceId, env.from);
    handlers.forEach((h) => {
      try {
        h(spaceId, env, fromMember);
      } catch (e) {
        console.error("[signal] kind handler error:", e);
      }
    });
  });
}

/**
 * 发送信令
 * @param to 目标虚拟 IP；缺省广播到所有 peers
 */
export async function sendSignal(
  spaceId: string,
  kind: SignalKind,
  type: string,
  data?: unknown,
  to?: string
): Promise<void> {
  const from = getSelfVirtualIp(spaceId);
  const env: SignalEnvelope = { kind, type, from: from ?? "", ...(to ? { to } : {}), ...(data !== undefined ? { data } : {}) };
  await sendSignalApi(spaceId, JSON.stringify(env), to);
}
