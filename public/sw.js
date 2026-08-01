// homeTier Service Worker
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