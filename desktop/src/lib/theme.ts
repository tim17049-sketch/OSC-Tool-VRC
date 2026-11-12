import type { Theme } from "./types";

type Palette = {
  ink: string;
  panel: string;
  raised: string;
  line: string;
  lineHover: string;
  text: string;
  muted: string;
  signal: string;
  warn: string;
  accent: string;
  /** The in-game look behind the chatbox preview. */
  sceneTop: string;
  sceneBottom: string;
  figure: string;
  trace: string;
  scheme: "dark" | "light";
};

export const PALETTES: Record<Theme, Palette> = {
  night: {
    ink: "#15122a",
    panel: "#1d1938",
    raised: "#272247",
    line: "#37305f",
    lineHover: "#4b4380",
    text: "#f1edfa",
    muted: "#a69fc8",
    signal: "#6ee7c8",
    warn: "#ffc66b",
    accent: "#ff5c8a",
    sceneTop: "#100e22",
    sceneBottom: "#1a1636",
    figure: "#322b5c",
    trace: "#191530",
    scheme: "dark",
  },
  dark: {
    ink: "#121316",
    panel: "#1a1b20",
    raised: "#24262d",
    line: "#33363f",
    lineHover: "#4a4e5a",
    text: "#f0f1f4",
    muted: "#a0a4b0",
    signal: "#6ee7c8",
    warn: "#ffc66b",
    accent: "#ff5c8a",
    sceneTop: "#0e0f12",
    sceneBottom: "#1c1e24",
    figure: "#30333c",
    trace: "#16171b",
    scheme: "dark",
  },
  amoled: {
    ink: "#000000",
    panel: "#0b0b0d",
    raised: "#17171a",
    line: "#27272c",
    lineHover: "#3d3d44",
    text: "#f4f4f5",
    muted: "#9c9ca5",
    signal: "#6ee7c8",
    warn: "#ffc66b",
    accent: "#ff5c8a",
    sceneTop: "#000000",
    sceneBottom: "#0e0e12",
    figure: "#232329",
    trace: "#060607",
    scheme: "dark",
  },
  light: {
    ink: "#f3f1f8",
    panel: "#ffffff",
    raised: "#eeebf6",
    line: "#dcd6ea",
    lineHover: "#bdb4d6",
    text: "#1c1830",
    muted: "#5f5878",
    signal: "#0b8a6e",
    warn: "#a35f00",
    accent: "#e0306a",
    // The preview stays a dark game scene: that is how VRChat looks.
    sceneTop: "#100e22",
    sceneBottom: "#1a1636",
    figure: "#322b5c",
    trace: "#f7f4fb",
    scheme: "light",
  },
};

export const ACCENTS = [
  "#ff5c8a",
  "#f43f5e",
  "#fb923c",
  "#facc15",
  "#4ade80",
  "#22d3ee",
  "#60a5fa",
  "#a78bfa",
];

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Dark or light text, whichever reads better on `hex`. */
export function onColor(hex: string): string {
  return luminance(hex) > 0.28 ? "#1a0f1c" : "#ffffff";
}

export function applyTheme(theme: Theme, accent: string) {
  const p = PALETTES[theme] ?? PALETTES.night;
  const color = /^#[0-9a-f]{6}$/i.test(accent) ? accent : p.accent;
  const vars: Record<string, string> = {
    "--ink": p.ink,
    "--panel": p.panel,
    "--raised": p.raised,
    "--line": p.line,
    "--line-hover": p.lineHover,
    "--text": p.text,
    "--muted": p.muted,
    "--signal": p.signal,
    "--warn": p.warn,
    "--pulse": color,
    "--on-pulse": onColor(color),
    "--scene-top": p.sceneTop,
    "--scene-bottom": p.sceneBottom,
    "--figure": p.figure,
    "--trace": p.trace,
  };
  const root = document.documentElement;
  for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);
  root.style.colorScheme = p.scheme;
  root.dataset.theme = theme;
}
