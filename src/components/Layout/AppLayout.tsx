import { ReactNode } from "react";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import { AppWorkspace } from "../AppBrowser/AppWorkspace";
import { useLayoutStore } from "../../stores/layoutStore";
import { motion, AnimatePresence } from "framer-motion";

interface AppLayoutProps {
  children: ReactNode;
}

export function AppLayout({ children }: AppLayoutProps) {
  const { sidebarOpen, setSidebarOpen } = useLayoutStore();

  return (
    <div className="h-screen w-screen flex flex-col bg-[var(--color-bg)]">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden relative">
        {/* 浮层 Sidebar — fixed 定位，不参与流式布局 */}
        <AnimatePresence>
          {sidebarOpen && (
            <>
              {/* 半透明遮罩层 */}
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.2 }}
                className="fixed inset-0 z-30 bg-black/50"
                onClick={() => setSidebarOpen(false)}
              />
              {/* Sidebar 面板 — 从左侧滑入 */}
              <motion.aside
                initial={{ x: "-100%" }}
                animate={{ x: 0 }}
                exit={{ x: "-100%" }}
                transition={{ type: "spring", damping: 25, stiffness: 260 }}
                className="fixed left-0 top-10 bottom-0 z-40 w-64 bg-[var(--color-surface)] border-r border-[var(--color-border)]"
              >
                <Sidebar />
              </motion.aside>
            </>
          )}
        </AnimatePresence>

        <main className="flex flex-col flex-1 overflow-hidden relative">
          {children}
          <AppWorkspace />
        </main>
      </div>
    </div>
  );
}