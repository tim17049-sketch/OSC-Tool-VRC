import { backend } from "./backend";
import type { Config, DeviceInfo, Snapshot } from "./types";

export type Page = "overview" | "chatbox" | "pulse" | "scanner" | "avatar" | "settings";

interface HistoryPoint {
  at: number;
  bpm: number;
}

const HISTORY_MS = 5 * 60 * 1000;
const RECENT_KEY = "osctool.recentMessages";
const RECENT_MAX = 8;

function loadRecent(): string[] {
  try {
    const list = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((m) => typeof m === "string").slice(0, RECENT_MAX) : [];
  } catch {
    return [];
  }
}

export const app = $state({
  page: "overview" as Page,
  config: null as Config | null,
  snap: null as Snapshot | null,
  history: [] as HistoryPoint[],
  /** Messages typed recently, newest first; kept on this PC only. */
  recent: loadRecent(),
  /** Set when the app could not start or settings could not be saved. */
  error: null as string | null,
});

function receive(snapshot: Snapshot) {
  app.snap = snapshot;
  const now = Date.now();
  const last = app.history[app.history.length - 1];
  if (snapshot.hr.bpm !== null && (!last || now - last.at >= 900)) {
    app.history.push({ at: now, bpm: snapshot.hr.bpm });
  }
  while (app.history.length && now - app.history[0].at > HISTORY_MS) {
    app.history.shift();
  }
}

export async function init() {
  try {
    app.config = await backend.getConfig();
    receive(await backend.getSnapshot());
    await backend.onSnapshot(receive);
  } catch (e) {
    app.error = String(e);
  }
}

function saveRecent() {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(app.recent));
  } catch {
    // Not being able to remember them is no problem.
  }
}

export function rememberMessage(text: string) {
  const trimmed = text.trim();
  if (!trimmed) return;
  app.recent = [trimmed, ...app.recent.filter((m) => m !== trimmed)].slice(0, RECENT_MAX);
  saveRecent();
}

export function forgetMessages() {
  app.recent = [];
  saveRecent();
}

/** Name to show for the connected device, whatever it reported. */
export function deviceName(device: DeviceInfo | null, fallback: string): string {
  return device?.name ?? fallback;
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

/** Change settings. They apply and save themselves a moment later. */
export function updateConfig(patch: Partial<Config>) {
  if (!app.config) return;
  app.config = { ...app.config, ...patch };
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    if (!app.config) return;
    try {
      await backend.setConfig($state.snapshot(app.config));
      app.error = null;
    } catch (e) {
      app.error = String(e);
    }
  }, 150);
}
