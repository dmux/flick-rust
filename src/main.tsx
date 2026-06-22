import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./App.css";

const api = {
  getConfig: () => invoke("get_config"),
  scanHid: () => invoke("scan_hid"),
  updateConfig: (config: any) => invoke("update_config", { config }),
  listApps: () => invoke("list_apps"),
  getAppVersion: () => invoke("get_app_version"),
  openExternal: (url: string) => invoke("open_external", { url }),
  launchApp: (execCmd: string) => invoke("launch_app", { execCmd }),
  resizeWindow: (width: number, height: number, mode: "launcher" | "settings") =>
    invoke("resize_window", { width, height, mode }),
  hideWindow: () => invoke("hide_window"),
  minimizeWindow: () => invoke("minimize_window"),
  onNavigate: (callback: (route: "launcher" | "settings") => void) => {
    listen("navigate", (event) => callback(event.payload as any));
  },
  onGetCurrentView: (callback: () => void) => {
    listen("get-current-view", () => callback());
  },
  sendCurrentView: (view: "launcher" | "settings") => {
    invoke("current_view_response", { view });
  }
};

(window as any).api = api;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
