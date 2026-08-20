import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { logEvent } from "./lib/api";
import "./index.css";

// Forward uncaught frontend errors and unhandled promise rejections to the
// backend log file so they're visible in release builds (no dev console).
// Best-effort: never let logging itself take down the app.
function reportFrontendError(kind: "error" | "warn", message: string) {
  if (message.length === 0) return;
  logEvent(kind, message).catch(() => {
    /* backend unavailable (e.g. web-only preview) — nothing to log to */
  });
}

window.addEventListener("error", (event) => {
  reportFrontendError("error", `window error: ${event.message}`);
});
window.addEventListener("unhandledrejection", (event) => {
  const reason = event.reason;
  let message = "unhandled rejection";
  if (reason instanceof Error) message = `unhandled rejection: ${reason.message}`;
  else if (typeof reason === "string" && reason) message = `unhandled rejection: ${reason}`;
  reportFrontendError("error", message);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
