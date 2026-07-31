import { type as osType } from "@tauri-apps/plugin-os";

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
