import { Component, ReactNode, useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import { AppWorkspace } from "../AppBrowser/AppWorkspace";
import { VoiceAutoJoin } from "../Voice/VoiceAutoJoin";
import { ShortcutOsd } from "../Common/ShortcutOsd";
import { useLayoutStore } from "../../stores/layoutStore";
import { useAppTabsStore } from "../../stores/appTabsStore";
import { motion, AnimatePresence } from "framer-motion";

interface AppLayoutProps {
  children: ReactNode;
}

const APP_ROUTE_RE = /^\/space\/[^/]+\/app\/[^/]+$/;

/** AppWorkspace 错误边界：React 渲染异常时给出恢复 UI，避免整树黑屏无法返回 */
class WorkspaceErrorBoundary extends Component<
  { children: ReactNode; onBack: () => void },
  { hasError: boolean }
> {
  state = { hasError: false };
  static getDerivedStateFromError() {
    return { hasError: true };
  }
  render() {
    if (this.state.hasError) {
      return (
        <div className="absolute inset-0 z-20 flex flex-col items-center justify-center gap-3 bg-[var(--color-bg)]">
          <div className="text-sm text-[var(--color-text-secondary)]">应用视图异常</div>
          <div className="flex gap-2">
            <button
              onClick={() => this.setState({ hasError: false })}
              className="px-4 py-2 rounded-lg bg-[var(--color-primary)] text-white text-sm"
            >
              重新加载
            </button>
            <button
              onClick={this.props.onBack}
              className="px-4 py-2 rounded-lg border border-[var(--color-border)] text-sm"
            >
              返回
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

export function AppLayout({ children }: AppLayoutProps) {
  const { sidebarOpen, setSidebarOpen } = useLayoutStore();
  const location = useLocation();
  const navigate = useNavigate();
  const hideWorkspace = useAppTabsStore((s) => s.hide);

  // 离开应用页（空间/设置/其他路由）时隐藏 AppWorkspace，避免其 z-20 浮层遮挡新页面
  useEffect(() => {
    if (!APP_ROUTE_RE.test(location.pathname)) {
      hideWorkspace();
    }
  }, [location.pathname, hideWorkspace]);

  return (
    <div className="h-full w-full flex flex-col bg-[var(--color-bg)]">
      <TitleBar />
      <div className="relative flex flex-1 overflow-hidden">
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
                className="fixed left-0 top-0 bottom-0 z-40 w-64 bg-[var(--color-bg)] border-r border-[var(--color-border)]"
              >
                <Sidebar />
              </motion.aside>
            </>
          )}
        </AnimatePresence>

        <main className="relative flex flex-col flex-1 overflow-hidden bg-[var(--color-bg)]">
          {children}
          <WorkspaceErrorBoundary onBack={() => navigate(-1)}>
            <AppWorkspace />
          </WorkspaceErrorBoundary>
          <VoiceAutoJoin />
          <ShortcutOsd />
        </main>
      </div>
    </div>
  );
}