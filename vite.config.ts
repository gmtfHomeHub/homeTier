import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(({ mode }) => {
  // 读取 .env 中的 VITE_PUBLIC_BASE（默认 /，部署到子路径时改为如 /hometier/）
  // base 必须以 / 结尾，保证 import.meta.env.BASE_URL 带尾斜杠（拼接子资源不丢路径分隔）
  const env = loadEnv(mode, process.cwd(), "");
  let publicBase = env.VITE_PUBLIC_BASE || "/";
  publicBase = publicBase.trim();
  if (publicBase !== "/" && !publicBase.endsWith("/")) publicBase += "/";
  if (publicBase !== "/" && !publicBase.startsWith("/") && !publicBase.startsWith(".")) {
    publicBase = "/" + publicBase;
  }

  return {
    plugins: [react()],
    base: publicBase,
    clearScreen: false,
    server: {
      port: 1420,
      strictPort: true,
      // host: 'localhost', // 仅监听本地回环接口
      watch: {
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});