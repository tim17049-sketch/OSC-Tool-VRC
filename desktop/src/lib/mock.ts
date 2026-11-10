import {
  CHATBOX_MAX_CHARS,
  DEFAULT_SCAN_PARAM,
  DEFAULT_TEMPLATE,
  MIN_INTERVAL_MS,
  type Backend,
  type Config,
  type MonitorRow,
  type ScanResult,
  type Snapshot,
} from "./types";

/** Browser-only stand-in for the Rust engine. `?watch=off` simulates a
 *  missing watch, `?music=off` silence, `?watch=old` an old watch app that
 *  only reports a model code. */
export function createMockBackend(): Backend {
  const params = new URLSearchParams(location.search);
  const watch = params.get("watch") !== "off";
  const oldWatch = params.get("watch") === "old";
  const music = params.get("music") !== "off";

  let config: Config = {
    oscHost: "127.0.0.1",
    oscPort: 9000,
    chatboxEnabled: true,
    template: DEFAULT_TEMPLATE,
    intervalMs: 2000,
    notifySound: false,
    messageSeconds: 10,
    messageKeep: false,
    time24h: true,
    showPaused: false,
    oscReceive: params.get("receive") === "on",
    oscReceivePort: 9001,
    paramFavorites: [
      { name: "Ears", kind: "bool", value: 1 },
      { name: "Outfit", kind: "int", value: 2 },
    ],
    scanName: "",
    scanParamEnabled: false,
    scanParam: DEFAULT_SCAN_PARAM,
    hrPort: 9555,
    hrAvatarParams: true,
    hrMin: 0,
    hrMax: 255,
    hrTimeoutS: 10,
    language: "auto",
    theme: "night",
    accent: "",
  };
  const started = Date.now();
  const samples: number[] = [];
  let message: { text: string; until: number | null } | null = null;
  const scans: ScanResult[] = [];
  let scanningUntil = 0;
  const monitor = new Map<string, MonitorRow & { at: number }>();
  let received = 0;
  const handlers = new Set<(s: Snapshot) => void>();

  const bpmNow = () => {
    const t = (Date.now() - started) / 1000;
    return Math.round(96 + 22 * Math.sin(t / 14) + 5 * Math.sin(t / 2.3));
  };

  const lyrics = ["Waiting in a car", "Waiting for a ride in the dark", "♪", "The city is my church"];
  const fmt = (ms: number) => `${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}`;

  const render = (values: Record<string, string | null>) =>
    config.template
      .split("\n")
      .map((line) => {
        let missing = false;
        const out = line.replace(/\{(\w+)\}/g, (all, name: string) => {
          if (!(name in values)) return all;
          if (values[name] === null) missing = true;
          return values[name] ?? "";
        });
        return missing ? null : out.trimEnd();
      })
      .filter((line): line is string => line !== null)
      .join("\n")
      .trim()
      .slice(0, CHATBOX_MAX_CHARS);

  const receive = (address: string, value: string) => {
    const row = monitor.get(address);
    received++;
    monitor.set(address, { address, value, count: (row?.count ?? 0) + 1, ageMs: 0, at: Date.now() });
  };

  const showMessage = (text: string, seconds: number | null) => {
    const trimmed = text.trim().slice(0, CHATBOX_MAX_CHARS);
    message = trimmed ? { text: trimmed, until: seconds === null ? null : Date.now() + seconds * 1000 } : null;
  };

  const snapshot = (): Snapshot => {
    const bpm = watch ? bpmNow() : null;
    if (bpm !== null) samples.push(bpm);
    const older = samples[samples.length - 8];
    const trend = bpm === null || older === undefined ? null : Math.sign(Math.round((bpm - older) / 3));
    const now = new Date();
    const time = now.toLocaleTimeString(config.time24h ? "de-DE" : "en-US", {
      hour: config.time24h ? "2-digit" : "numeric",
      minute: "2-digit",
    });
    const positionMs = ((Date.now() - started) % 244_000) + 30_000;
    const media = music
      ? {
          title: "Midnight City",
          artist: "M83",
          album: "Hurry Up, We're Dreaming",
          playing: true,
          positionMs,
          durationMs: 244_000,
        }
      : null;
    if (message?.until && Date.now() >= message.until) message = null;
    if (config.oscReceive) {
      receive("/avatar/parameters/VelocityX", (Math.random() * 0.2).toFixed(3));
      receive("/avatar/parameters/Grounded", "true");
      receive("/avatar/parameters/Ears", "true");
      receive("/avatar/parameters/Outfit", "2");
      receive("/avatar/change", '"avtr_3f1c…"');
    }
    const filled = Math.round((positionMs / 244_000) * 9);
    const values: Record<string, string | null> = {
      hr: bpm?.toString() ?? null,
      hr_trend: bpm === null ? null : trend === 1 ? "↑" : trend === -1 ? "↓" : trend === 0 ? "→" : "",
      heart: bpm === null ? null : samples.length % 2 ? "❤" : "♡",
      hr_min: bpm === null ? null : Math.min(...samples).toString(),
      hr_max: bpm === null ? null : Math.max(...samples).toString(),
      hr_avg: bpm === null ? null : Math.round(samples.reduce((a, b) => a + b, 0) / samples.length).toString(),
      watch: watch ? "Galaxy Watch7" : null,
      watch_battery: watch ? "81%" : null,
      song: media ? `${media.artist} – ${media.title}` : null,
      title: media?.title ?? null,
      artist: media?.artist ?? null,
      album: media?.album ?? null,
      play: media ? "▶" : null,
      bar: media ? "━".repeat(filled) + "●" + "─".repeat(9 - filled) : null,
      pos: media ? fmt(positionMs) : null,
      dur: media ? fmt(244_000) : null,
      lyric: media ? lyrics[Math.floor(positionMs / 5000) % lyrics.length] : null,
      time,
      date: now.toLocaleDateString(config.language === "en" ? "sv-SE" : "de-DE"),
      cpu: "23%",
      ram: "61%",
    };
    return {
      hr: {
        bpm,
        connected: watch,
        device: !watch
          ? null
          : oldWatch
            ? {
                name: "Galaxy Watch5",
                manufacturer: null,
                model: null,
                os: null,
                app: null,
                sensor: null,
                battery: null,
                charging: null,
              }
            : {
                name: "Galaxy Watch7",
                manufacturer: "samsung",
                model: "SM-L310",
                os: "Wear OS 5",
                app: "0.2.0",
                sensor: "Samsung HR Sensor",
                battery: 81,
                charging: false,
              },
        trend,
        min: samples.length ? Math.min(...samples) : null,
        max: samples.length ? Math.max(...samples) : null,
        avg: samples.length ? Math.round(samples.reduce((a, b) => a + b, 0) / samples.length) : null,
        sampleAgeMs: watch ? 300 : null,
      },
      chatbox: {
        preview: message?.text ?? render(values),
        enabled: config.chatboxEnabled,
        sentCount: Math.floor((Date.now() - started) / config.intervalMs),
        lastSentMsAgo: 800,
        messageRemainingS: message ? (message.until ? Math.ceil((message.until - Date.now()) / 1000) : 0) : null,
        messageKept: message !== null && message.until === null,
        error: null,
      },
      media,
      lyrics: media && config.template.includes("{lyric}") ? "found" : null,
      cpu: 23,
      ram: 61,
      network: {
        hostName: "GAMING-PC",
        addresses: ["192.168.178.42"],
        hrPort: config.hrPort,
        hrListening: true,
        discoverable: true,
        hrError: null,
      },
      receiver: {
        enabled: config.oscReceive,
        port: config.oscReceivePort,
        listening: config.oscReceive,
        error: null,
        total: received,
      },
      scanner: { scanning: Date.now() < scanningUntil, results: [...scans] },
    };
  };

  const emit = () => {
    const s = snapshot();
    handlers.forEach((h) => h(s));
  };
  setInterval(emit, 1000);

  return {
    getConfig: async () => ({ ...config }),
    setConfig: async (next) => {
      config = { ...next, intervalMs: Math.max(MIN_INTERVAL_MS, next.intervalMs) };
      queueMicrotask(emit);
      return { ...config };
    },
    getSnapshot: async () => snapshot(),
    sendMessage: async (text) => {
      showMessage(text, config.messageKeep ? null : config.messageSeconds);
      emit();
    },
    setTyping: async () => {},
    scan: async (name) => {
      if (Date.now() < scanningUntil) return false;
      const who = name?.trim() || config.scanName || "Du";
      scanningUntil = Date.now() + 2600;
      showMessage(`🔍 Scanne ${who} …\nMesse Flauschfaktor …`, 5);
      emit();
      setTimeout(() => {
        const percent = Math.floor(Math.random() * 101);
        const verdict = percent > 55 ? "Eindeutig flauschig. Leugnen zwecklos." : "Halb Fell, halb Ausreden.";
        const time = new Date().toLocaleTimeString("de-DE");
        scans.unshift({ time, name: who, percent, verdict });
        scans.splice(15);
        showMessage(`🐾 ${who}: ${percent} % Furry\n${verdict}\nBeweis: hat beim Scan gewedelt`, 10);
        emit();
      }, 2600);
      return true;
    },
    sendParam: async (name, kind, value) => {
      if (!name.trim() || /\s/.test(name.trim())) throw "invalid parameter name";
      console.info("param", name, kind, value);
    },
    getMonitor: async () =>
      [...monitor.values()]
        .map(({ at, ...row }) => ({ ...row, ageMs: Date.now() - at }))
        .sort((a, b) => a.address.localeCompare(b.address)),
    clearMonitor: async () => monitor.clear(),
    onSnapshot: async (handler) => {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
  };
}
