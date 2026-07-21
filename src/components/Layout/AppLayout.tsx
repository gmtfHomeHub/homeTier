import { ReactNode, useEffect, useState } from "react";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import { useLayoutStore } from "../../stores/layoutStore";
import { motion, AnimatePresence } from "framer-motion";

interface AppLayoutProps {
  children: ReactNode;
}

function useIsMobile() {
  const [isMobile, setIsMobile] = useState(
    () => window.matchMedia("(max-width: 768px)").matches
  );
  useEffect(() => {
    const mq = window.matchMedia("(max-width: 768px)");
    const handler = (e: MediaQueryListEvent) => setIsMobile(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);
  return isMobile;
}

export function AppLayout({ children }: AppLayoutProps) {
  const { sidebarOpen, setSidebarOpen } = useLayoutStore();
  const isMobile = useIsMobile();

  return (
    <div className="h-screen w-screen flex flex-col bg-[var(--color-bg)]">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden relative">
        {/* Sidebar 容器 — 宽度动画 */}
        <motion.div
          animate={{ width: sidebarOpen ? 256 : 0 }}
          transition={{ duration: 0.25, ease: "easeInOut" }}
          className="overflow-hidden shrink-0"
        >
          <Sidebar />
        </motion.div>

        {/* 移动端遮罩层 */}
        <AnimatePresence>
          {sidebarOpen && isMobile && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="fixed inset-0 z-10 bg-black/50"
              onClick={() => setSidebarOpen(false)}
            />
          )}
        </AnimatePresence>

        <main className="flex flex-col flex-1 overflow-hidden">
          {children}
        </main>
      </div>
    </div>
  );
}