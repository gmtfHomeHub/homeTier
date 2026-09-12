// src/utils/share.ts - 分享链接解析助手（join_space 专用）
import { parseQR, parseShareData } from "./api";
import type { ShareInfo } from "../types";

/**
 * join_space 事件标识，与后端 `crate::qr::EVENT_JOIN_SPACE` 保持一致。
 * 两侧需手动同步（参见 AGENTS.md 类型同步约定）。
 */
export const QR_EVENT_JOIN_SPACE = "j_s";

/**
 * add_app 事件标识，与后端 `crate::qr::EVENT_ADD_APP` 保持一致。
 */
export const QR_EVENT_ADD_APP = "a_a";

/**
 * 解析二维码文本为 `ShareInfo`（仅接受 `j_s` 事件）。
 *
 * - 事件不是 `j_s` 时抛错（由调用方 catch + toast）。
 * - 链接损坏 / 解密失败同样抛错。
 */
export async function resolveJoinShareInfo(text: string): Promise<ShareInfo> {
  const { event, data } = await parseQR(text);
  if (event !== QR_EVENT_JOIN_SPACE) {
    throw new Error(`unsupported QR event: ${event}`);
  }
  return parseShareData(data);
}
