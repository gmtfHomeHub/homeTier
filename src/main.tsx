/* eslint-disable react-refresh/only-export-components */
import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import { createPortal } from "react-dom";
import App from "./App";
import { Toaster } from "react-hot-toast";
import { useIsMobile } from "./utils/device";
import "@radix-ui/themes/styles.css";
import "./styles/globals.css";
import "./i18n";

// 注册 Service Worker
if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker
      .register("/sw.js")
      .then(() => console.log("SW registered"))
      .catch((err) => console.warn("SW registration failed:", err));
  });
}

function Root() {
  const isMobile = useIsMobile();

  // 移动端软键盘：visualViewport 缩小时动态上推 #root 底部，使其跟随可视区；
  // 聚焦输入框时滚动到可见区中心，避免被键盘遮挡
  useEffect(() => {
    if (!isMobile) return;
    const root = document.getElementById("root");
    const vv = window.visualViewport;
    if (!root || !vv) return;

    const scrollFocused = () => {
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable)) {
        el.scrollIntoView({ block: "center", behavior: "smooth" });
      }
    };
    const onResize = () => {
      // 键盘高度 = 内屏高 - 可视区高 - 顶部偏移；上推 #root 底部避开键盘
      const kb = window.innerHeight - vv.height - vv.offsetTop;
      root.style.bottom = `${Math.max(kb, 0)}px`;
      // Dialog Content（portal 到 body，不受 #root 上推影响）：
      // 高度跟随可视区，配合 [role=dialog] overflow-y:auto 使内部 scrollIntoView 生效，
      // 避免弹窗内输入框被键盘遮挡
      const vh = vv.height - vv.offsetTop;
      document.querySelectorAll<HTMLElement>('[role="dialog"]').forEach((el) => {
        el.style.maxHeight = `${Math.max(vh - 16, 200)}px`;
      });
      scrollFocused();
    };
    const onFocusIn = (e: FocusEvent) => {
      const t = e.target as HTMLElement;
      if (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable) {
        setTimeout(scrollFocused, 300);
      }
    };
    vv.addEventListener("resize", onResize);
    document.addEventListener("focusin", onFocusIn);
    onResize();
    return () => {
      vv.removeEventListener("resize", onResize);
      document.removeEventListener("focusin", onFocusIn);
      root.style.bottom = "";
      document.querySelectorAll<HTMLElement>('[role="dialog"]').forEach((el) => {
        el.style.maxHeight = "";
      });
    };
  }, [isMobile]);

  return (
    <>
      <React.StrictMode>
        <App />
      </React.StrictMode>
      {createPortal(
        <Toaster
          position={isMobile ? "top-center" : "top-right"}
          containerStyle={{ zIndex: 99999 }}
          toastOptions={{
            style: {
              ...(isMobile
                ? {
                    width: "calc(100vw - 2rem)",
                    paddingTop: "env(safe-area-inset-top)",
                  }
                : {}),
            },
          }}
        />,
        document.body
      )}
    </>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <Root />
);