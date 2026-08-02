import { type as osType } from "@tauri-apps/plugin-os";
import { useEffect, useState } from "react";

export type DeviceMode = "desktop" | "mobile";

export const DEVICE_VIEWPORTS: Record<DeviceMode, { w: number; h: number }> = {
  desktop: { w: 1920, h: 1080 },
  mobile: { w: 390, h: 844 },
};

const MOBILE_UA = /android|iphone|ipad|ipod|mobile/i;

export function detectDeviceMode(): DeviceMode {
  try {
    const t = osType();
    if (t === "ios" || t === "android") return "mobile";
    return "desktop";
  } catch {
    // 非 Tauri 环境（如浏览器 dev），回退到 UA 判断
    return MOBILE_UA.test(navigator.userAgent) ? "mobile" : "desktop";
  }
}

const MOBILE_MQ = "(max-width: 768px)";

/** 判断当前视口是否为移动端宽度（同步查询） */
export function isMobile(): boolean {
  return window.matchMedia(MOBILE_MQ).matches;
}

/** 响应式移动端检测 hook：视口跨断点时实时更新 */
export function useIsMobile(): boolean {
  const [mobile, setMobile] = useState<boolean>(() => isMobile());

  useEffect(() => {
    const mq = window.matchMedia(MOBILE_MQ);
    const handler = (e: MediaQueryListEvent) => setMobile(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  return mobile;
}
