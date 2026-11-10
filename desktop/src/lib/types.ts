// Mirrors the Rust structs in core/src/config.rs and core/src/engine.rs.

export type ParamKind = "bool" | "int" | "float";
export type Theme = "night" | "dark" | "amoled" | "light";

export interface ParamFavorite {
  name: string;
  kind: ParamKind;
  value: number;
}

export interface DeviceInfo {
  name: string | null;
  manufacturer: string | null;
  model: string | null;
  os: string | null;
  app: string | null;
  sensor: string | null;
  battery: number | null;
  charging: boolean | null;
}

export interface Media {
  title: string;
  artist: string;
  album: string;
  playing: boolean;
  positionMs: number | null;
  durationMs: number | null;
}

export interface ScanResult {
  time: string;
  name: string;
  percent: number;
  verdict: string;
}

export interface MonitorRow {
  address: string;
  value: string;
  count: number;
  ageMs: number;
}

export interface Config {
  oscHost: string;
  oscPort: number;
  chatboxEnabled: boolean;
  template: string;
  intervalMs: number;
  notifySound: boolean;
  messageSeconds: number;
  messageKeep: boolean;
  time24h: boolean;
  showPaused: boolean;
  oscReceive: boolean;
  oscReceivePort: number;
  paramFavorites: ParamFavorite[];
  scanName: string;
  scanParamEnabled: boolean;
  scanParam: string;
  hrPort: number;
  hrAvatarParams: boolean;
  hrMin: number;
  hrMax: number;
  hrTimeoutS: number;
  language: "auto" | "de" | "en";
  theme: Theme;
  /** "#rrggbb" or "" for the theme's own accent. */
  accent: string;
}

export interface Snapshot {
  hr: {
    bpm: number | null;
    connected: boolean;
    device: DeviceInfo | null;
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
    messageKept: boolean;
    error: string | null;
  };
  media: Media | null;
  lyrics: "loading" | "found" | "missing" | "error" | null;
  cpu: number;
  ram: number;
  network: {
    hostName: string;
    addresses: string[];
    hrPort: number;
    hrListening: boolean;
    discoverable: boolean;
    hrError: string | null;
  };
  receiver: {
    enabled: boolean;
    port: number;
    listening: boolean;
    error: string | null;
    total: number;
  };
  scanner: {
    scanning: boolean;
    results: ScanResult[];
  };
}

export interface Backend {
  getConfig(): Promise<Config>;
  setConfig(config: Config): Promise<Config>;
  getSnapshot(): Promise<Snapshot>;
  sendMessage(text: string): Promise<void>;
  setTyping(typing: boolean): Promise<void>;
  scan(name?: string): Promise<boolean>;
  sendParam(name: string, kind: ParamKind, value: number): Promise<void>;
  getMonitor(): Promise<MonitorRow[]>;
  clearMonitor(): Promise<void>;
  onSnapshot(handler: (snapshot: Snapshot) => void): Promise<() => void>;
}

export const CHATBOX_MAX_CHARS = 144;
export const CHATBOX_MAX_LINES = 9;
export const MIN_INTERVAL_MS = 1500;
export const DEFAULT_SCAN_PARAM = "FurryScan";
export const DEFAULT_TEMPLATE = "❤ {hr} bpm {hr_trend}\n🎵 {song}\n🕐 {time}";
