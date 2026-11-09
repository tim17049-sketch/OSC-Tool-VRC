import { backend } from "./backend";
import type { Config, Snapshot } from "./types";

export type Page = "overview" | "chatbox" | "pulse" | "settings";

interface HistoryPoint {
  at: number;
  bpm: number;
}

const HISTORY_MS = 5 * 60 * 1000;

export const app = $state({
  page: "overview" as Page,
  config: null as Config | null,
  snap: null as Snapshot | null,
  history: [] as HistoryPoint[],
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
