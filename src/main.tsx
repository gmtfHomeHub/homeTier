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

  // 移动端软键盘弹起时，自动滚动聚焦的输入框到可见区，避免被键盘遮挡
  useEffect(() => {
    if (!isMobile) return;
    const handler = (e: FocusEvent) => {
      const t = e.target as HTMLElement;
      if (
        t.tagName === "INPUT" ||
        t.tagName === "TEXTAREA" ||
        t.isContentEditable
      ) {
        setTimeout(
          () => t.scrollIntoView({ block: "center", behavior: "smooth" }),
          300
        );
      }
    };
    document.addEventListener("focusin", handler);
    return () => document.removeEventListener("focusin", handler);
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