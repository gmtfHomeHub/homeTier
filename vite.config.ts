import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // host: 'localhost', // 仅监听本地回环接口
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
