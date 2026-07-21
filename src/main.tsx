import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
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

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);