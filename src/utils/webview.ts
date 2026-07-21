import { open } from "@tauri-apps/plugin-shell";
import { type } from "@tauri-apps/plugin-os";

// Tauri 环境检测
declare global {
  interface Window {
    __TAURI_INTERNALS__?: Record<string, unknown>;
  }
}

/** 当前是否运行在 Tauri 环境中 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && !!window.__TAURI_INTERNALS__;
}

/** 当前平台类型 */
export type PlatformType = "windows" | "macos" | "linux" | "android" | "ios" | "unknown";

/** 获取当前平台类型 */
export function getPlatform(): PlatformType {
  if (!isTauri()) return "unknown";
  try {
    const osType = type();
    if (osType === "windows") return "windows";
    if (osType === "macos") return "macos";
    if (osType === "linux") return "linux";
    if (osType === "android") return "android";
    if (osType === "ios") return "ios";
    return "unknown";
  } catch {
    return "unknown";
  }
}

/** 是否运行在移动端（Android / iOS） */
export function isMobile(): boolean {
  const platform = getPlatform();
  return platform === "android" || platform === "ios";
}

/** 是否运行在桌面端（Windows / macOS / Linux） */
export function isDesktop(): boolean {
  const platform = getPlatform();
  return platform === "windows" || platform === "macos" || platform === "linux";
}

/**
 * 在系统浏览器中打开 URL。
 * Tauri 环境使用 @tauri-apps/plugin-shell 的 open API
 * 非 Tauri 环境回退到 window.open
 */
export async function openInBrowser(url: string): Promise<void> {
  if (isTauri()) {
    try {
      await open(url);
      return;
    } catch {
      // fall through to window.open
    }
  }
  window.open(url, "_blank");
}