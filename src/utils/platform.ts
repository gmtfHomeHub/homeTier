// src/utils/platform.ts - 平台检测（tauri 模式走 plugin-os，web 模式走 UA）
import { platform as osPlatform } from "@tauri-apps/plugin-os";
import { isTauri } from "./api";

export type AppPlatform =
  | "windows"
  | "macos"
  | "linux"
  | "ios"
  | "android"
  | "web";

let cached: AppPlatform | null = null;
let cachePromise: Promise<AppPlatform> | null = null;

function detectFromUA(): AppPlatform {
  const ua = navigator.userAgent;
  if (/iPhone|iPad|iPod/i.test(ua)) return "ios";
  if (/Android/i.test(ua)) return "android";
  return "web";
}

export async function getPlatform(): Promise<AppPlatform> {
  if (cached) return cached;
  if (cachePromise) return cachePromise;
  cachePromise = (async () => {
    let p: AppPlatform;
    if (isTauri()) {
      try {
        p = (await osPlatform()) as AppPlatform;
      } catch {
        p = detectFromUA();
      }
    } else {
      p = detectFromUA();
    }
    cached = p;
    return p;
  })();
  return cachePromise;
}

export function isMobilePlatform(p: AppPlatform): boolean {
  return p === "ios" || p === "android";
}

export async function isMobile(): Promise<boolean> {
  return isMobilePlatform(await getPlatform());
}
