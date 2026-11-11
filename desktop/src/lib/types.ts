// Mirrors the Rust structs in core/src/config.rs and core/src/engine.rs.

export interface Config {
  oscHost: string;
  oscPort: number;
  chatboxEnabled: boolean;
  template: string;
  intervalMs: number;
  notifySound: boolean;
  messageSeconds: number;
  time24h: boolean;
  hrPort: number;
  hrAvatarParams: boolean;
  hrMin: number;
  hrMax: number;
  hrTimeoutS: number;
  language: "auto" | "de" | "en";
}

export interface Snapshot {
  hr: {
    bpm: number | null;
    connected: boolean;
    device: string | null;
    trend: number | null;
    min: number | null;
    max: number | null;
    avg: number | null;
    sampleAgeMs: number | null;
  };
  chatbox: {
    preview: string;
    enabled: boolean;
    sentCount: number;
    lastSentMsAgo: number | null;
    messageRemainingS: number | null;
    error: string | null;
  };
  media: { title: string; artist: string } | null;
  cpu: number;
  ram: number;
  cpuTemp: number | null;
  gpu: number | null;
  gpuTemp: number | null;
  vr: { hmd: Battery | null; left: Battery | null; right: Battery | null };
  network: {
    hostName: string;
    addresses: string[];
    hrPort: number;
    hrListening: boolean;
    discoverable: boolean;
    hrError: string | null;
  };
}

export interface Battery {
  percent: number;
  charging: boolean;
}

export interface Backend {
  getConfig(): Promise<Config>;
  setConfig(config: Config): Promise<Config>;
  getSnapshot(): Promise<Snapshot>;
  sendMessage(text: string): Promise<void>;
  setTyping(typing: boolean): Promise<void>;
  onSnapshot(handler: (snapshot: Snapshot) => void): Promise<() => void>;
}

export const CHATBOX_MAX_CHARS = 144;
export const CHATBOX_MAX_LINES = 9;
export const MIN_INTERVAL_MS = 1500;
export const DEFAULT_TEMPLATE = "❤ {hr} bpm {hr_trend}\n🎵 {song}\n🕐 {time}";
