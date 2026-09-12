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

/** 判断平台是否为移动端（iOS/Android，同步，平台不变无需 effect）。
 * 区别于 useIsMobile（视口宽度断点）：此 hook 判断真实设备平台，
 * 用于区分桌面端原生 desktop 模式（需自适应 100%）vs 移动端切到 desktop 模式（需 50% + 手势）。 */
export function useIsMobilePlatform(): boolean {
  const [mobilePlatform] = useState<boolean>(() => {
    try {
      const t = osType();
      return t === "ios" || t === "android";
    } catch {
      return MOBILE_UA.test(navigator.userAgent);
    }
  });
  return mobilePlatform;
}
