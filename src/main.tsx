import React from "react";
import ReactDOM from "react-dom/client";
import { AppRoot } from "@/app/app-root";
import "@/styles/globals.css";

function preventUnsafeDesktopShortcuts() {
  const handleContextMenu = (event: MouseEvent) => {
    event.preventDefault();
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    const key = event.key.toLowerCase();
    const withCommand = event.ctrlKey || event.metaKey;
    const shouldBlock =
      event.key === "F5" ||
      event.key === "F12" ||
      (withCommand && key === "r") ||
      (withCommand && event.shiftKey && ["i", "j", "c"].includes(key));

    if (!shouldBlock) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
  };

  window.addEventListener("contextmenu", handleContextMenu);
  window.addEventListener("keydown", handleKeyDown, true);
}

preventUnsafeDesktopShortcuts();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppRoot />
  </React.StrictMode>
);
