import {
  CHATBOX_MAX_CHARS,
  DEFAULT_TEMPLATE,
  MIN_INTERVAL_MS,
  type Backend,
  type Config,
  type Snapshot,
} from "./types";

/** Browser-only stand-in for the Rust engine. `?watch=off` simulates a
 *  missing watch, `?music=off` silence. */
export function createMockBackend(): Backend {
  const params = new URLSearchParams(location.search);
  const watch = params.get("watch") !== "off";
  const music = params.get("music") !== "off";

  let config: Config = {
    oscHost: "127.0.0.1",
    oscPort: 9000,
    chatboxEnabled: true,
    template: DEFAULT_TEMPLATE,
    intervalMs: 2000,
    notifySound: false,
    messageSeconds: 10,
    time24h: true,
    hrPort: 9555,
    hrAvatarParams: true,
    hrMin: 0,
    hrMax: 255,
    hrTimeoutS: 10,
    language: "auto",
  };
  const started = Date.now();
  const samples: number[] = [];
  let message: { text: string; until: number } | null = null;
  const handlers = new Set<(s: Snapshot) => void>();

  const bpmNow = () => {
    const t = (Date.now() - started) / 1000;
    return Math.round(96 + 22 * Math.sin(t / 14) + 5 * Math.sin(t / 2.3));
  };

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
    const media = music ? { title: "Midnight City", artist: "M83" } : null;
    if (message && Date.now() >= message.until) message = null;
    const values: Record<string, string | null> = {
      hr: bpm?.toString() ?? null,
      hr_trend: bpm === null ? null : trend === 1 ? "↑" : trend === -1 ? "↓" : trend === 0 ? "→" : "",
      heart: bpm === null ? null : samples.length % 2 ? "❤" : "♡",
      hr_min: bpm === null ? null : Math.min(...samples).toString(),
      hr_max: bpm === null ? null : Math.max(...samples).toString(),
      hr_avg: bpm === null ? null : Math.round(samples.reduce((a, b) => a + b, 0) / samples.length).toString(),
      song: media ? `${media.artist} – ${media.title}` : null,
      title: media?.title ?? null,
      artist: media?.artist ?? null,
      time,
      date: now.toLocaleDateString(config.language === "en" ? "sv-SE" : "de-DE"),
      cpu: "23%",
      ram: "61%",
    };
    return {
      hr: {
        bpm,
        connected: watch,
        device: watch ? "Galaxy Watch7" : null,
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
        messageRemainingS: message ? Math.ceil((message.until - Date.now()) / 1000) : null,
        error: null,
      },
      media,
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
      const trimmed = text.trim().slice(0, CHATBOX_MAX_CHARS);
      message = trimmed ? { text: trimmed, until: Date.now() + config.messageSeconds * 1000 } : null;
      emit();
    },
    setTyping: async () => {},
    onSnapshot: async (handler) => {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
  };
}
