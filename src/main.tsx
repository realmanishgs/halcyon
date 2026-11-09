import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { loadSettings } from "./engine";

async function boot() {
  // Dev-only browser preview with a fake backend (see dev-mock.ts); stripped from production builds.
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock")) {
    (await import("./dev-mock")).installMock();
  }
  await loadSettings();
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
boot();
