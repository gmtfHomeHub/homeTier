// homeTier Service Worker
// 当前版本：保留注册，无代理逻辑
// 跨源 iframe 的 X-Frame-Options 限制无法通过 SW 代理解决（CORS 限制）
// 备选方案：在 Tauri 桌面端可使用 WebviewWindow（已移回 iframe 模式）
// 未来可在此处扩展离线缓存、推送通知等功能

self.addEventListener("install", () => {
  // 跳过等待，立即激活
  self.skipWaiting();
});

self.addEventListener("activate", () => {
  // 接管所有客户端
  self.clients.claim();
});

export {};