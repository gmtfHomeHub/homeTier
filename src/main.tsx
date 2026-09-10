import React from "react";
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