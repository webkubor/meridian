/**
 * Theme bridge for the canvas renderer.
 *
 * Canvas cannot consume CSS custom properties, so the tokens docs/04 §2 defines
 * are read out of `getComputedStyle` at runtime. Values are never duplicated:
 * lanes, bubble colours and badge colours all come from `tokens.css`, including
 * the `.theme-dark` overrides, so a theme change is just "read again".
 *
 * Fallbacks are CSS system colours (never literals) so a missing token degrades
 * to something visible instead of an invalid `fillStyle`.
 */

import { onScopeDispose, ref, type Ref } from "vue";

export interface GraphTheme {
  /** Bumped on every read; use it as a cache key so readers re-read on change. */
  version: number;
  /** True when the document carries `.theme-dark` (tokens.css' dark scope). */
  dark: boolean;
  /** Nine lane hues, `palette[lane % 9]` (docs/04 §2.5). */
  laneColors: string[];
  bubbleBg: string;
  bubbleBorder: string;
  badgeBg: string;
  badgeFg: string;
  accent: string;
  accentContrast: string;
  fgMuted: string;
  borderStrong: string;
  fontFamily: string;
  fontSizeXs: number;
  radiusSm: number;
  laneWidth: number;
  laneWidthCompact: number;
  bubbleRadius: number;
}

let version = 0;

export function readGraphTheme(): GraphTheme {
  if (typeof document === "undefined" || typeof window === "undefined") {
    return fallbackTheme();
  }
  const styles = getComputedStyle(document.documentElement);
  const read = (token: string, fallback: string): string =>
    styles.getPropertyValue(token).trim() || fallback;
  const pixels = (token: string, fallback: number): number => {
    const value = Number.parseFloat(styles.getPropertyValue(token));
    return Number.isFinite(value) ? value : fallback;
  };

  const laneColors: string[] = [];
  for (let lane = 1; lane <= 9; lane += 1) {
    laneColors.push(read(`--md-graph-lane-${lane}`, "GrayText"));
  }

  version += 1;
  return {
    version,
    dark: document.documentElement.classList.contains("theme-dark"),
    laneColors,
    bubbleBg: read("--md-graph-bubble-bg", "Canvas"),
    bubbleBorder: read("--md-graph-bubble-border", "GrayText"),
    badgeBg: read("--md-badge-bg", "Canvas"),
    badgeFg: read("--md-badge-fg", "CanvasText"),
    accent: read("--md-accent", "Highlight"),
    accentContrast: read("--md-accent-contrast", "CanvasText"),
    fgMuted: read("--md-fg-muted", "GrayText"),
    borderStrong: read("--md-border-strong", "GrayText"),
    fontFamily: read("--md-font-ui", "system-ui, sans-serif"),
    fontSizeXs: pixels("--md-text-xs", 11),
    radiusSm: pixels("--md-radius-sm", 5),
    laneWidth: pixels("--md-graph-lane-width", 14),
    laneWidthCompact: pixels("--md-graph-lane-width-compact", 10),
    bubbleRadius: pixels("--md-graph-bubble-radius", 4),
  };
}

function fallbackTheme(): GraphTheme {
  version += 1;
  return {
    version,
    dark: false,
    laneColors: Array.from({ length: 9 }, () => "GrayText"),
    bubbleBg: "Canvas",
    bubbleBorder: "GrayText",
    badgeBg: "Canvas",
    badgeFg: "CanvasText",
    accent: "Highlight",
    accentContrast: "CanvasText",
    fgMuted: "GrayText",
    borderStrong: "GrayText",
    fontFamily: "system-ui, sans-serif",
    fontSizeXs: 11,
    radiusSm: 5,
    laneWidth: 14,
    laneWidthCompact: 10,
    bubbleRadius: 4,
  };
}

/** `palette[lane % 9]` with a defensive fallback for negative/absent lanes. */
export function laneColor(theme: GraphTheme, lane: number): string {
  if (!Number.isFinite(lane) || lane < 0) return theme.fgMuted;
  return theme.laneColors[lane % theme.laneColors.length] ?? theme.fgMuted;
}

/**
 * Reactive theme for components: re-reads whenever the document's theme class
 * (or any inline token override) changes, or the OS appearance flips.
 * Must be called from a component scope.
 */
export function useGraphTheme(): Ref<GraphTheme> {
  const theme = ref(readGraphTheme());

  if (typeof document === "undefined" || typeof window === "undefined") {
    return theme;
  }

  const observer = new MutationObserver(() => {
    theme.value = readGraphTheme();
  });
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["class", "style", "data-theme"],
  });

  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const onAppearance = () => {
    theme.value = readGraphTheme();
  };
  media.addEventListener("change", onAppearance);

  onScopeDispose(() => {
    observer.disconnect();
    media.removeEventListener("change", onAppearance);
  });

  return theme;
}

/**
 * Composited alpha over a token colour. `#rrggbb`, `#rgb`, `rgb(...)` and
 * `rgba(...)` inputs are understood; anything else (system colours) is returned
 * unchanged, which keeps canvas `fillStyle` valid.
 */
export function alpha(color: string, alphaValue: number): string {
  const rgb = parseColor(color);
  if (!rgb) return color;
  const a = Math.max(0, Math.min(1, alphaValue));
  return `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${Math.round(a * 1000) / 1000})`;
}

function parseColor(color: string): [number, number, number] | null {
  const value = color.trim();
  if (value.startsWith("#")) {
    const hex = value.slice(1);
    if (hex.length === 3 || hex.length === 4) {
      const r = Number.parseInt(hex[0] + hex[0], 16);
      const g = Number.parseInt(hex[1] + hex[1], 16);
      const b = Number.parseInt(hex[2] + hex[2], 16);
      return Number.isFinite(r) && Number.isFinite(g) && Number.isFinite(b) ? [r, g, b] : null;
    }
    if (hex.length === 6 || hex.length === 8) {
      const r = Number.parseInt(hex.slice(0, 2), 16);
      const g = Number.parseInt(hex.slice(2, 4), 16);
      const b = Number.parseInt(hex.slice(4, 6), 16);
      return Number.isFinite(r) && Number.isFinite(g) && Number.isFinite(b) ? [r, g, b] : null;
    }
    return null;
  }
  const match = value.match(/^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)/i);
  if (match) {
    return [Number(match[1]), Number(match[2]), Number(match[3])];
  }
  return null;
}
