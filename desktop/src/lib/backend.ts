import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Backend, Config, Snapshot } from "./types";
import { createMockBackend } from "./mock";

const tauriBackend: Backend = {
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (config) => invoke<Config>("set_config", { config }),
  getSnapshot: () => invoke<Snapshot>("get_snapshot"),
  sendMessage: (text) => invoke("send_message", { text }),
  setTyping: (typing) => invoke("set_typing", { typing }),
  onSnapshot: (handler) => listen<Snapshot>("snapshot", (event) => handler(event.payload)),
};

/** Inside the app we talk to Rust; in a plain browser (`npm run dev`) a
 *  simulator stands in so the UI can be worked on without VRChat or a watch. */
export const isDesktop = "__TAURI_INTERNALS__" in window;
export const backend: Backend = isDesktop ? tauriBackend : createMockBackend();
