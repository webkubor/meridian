/**
 * Ref badge chips: layout + measurement, shared by the renderer (which paints
 * them on the row canvas) and the row component (which insets its text after
 * the strip). Measuring once and painting from the measured model keeps the
 * canvas and the DOM text in agreement.
 */

import { measureContext } from "./measure";
import type { RefBadge, RefKind } from "./refs";
import { alpha, type GraphTheme } from "./theme";

export interface BadgeChip {
  badge: RefBadge;
  /** Display label (already ellipsized to fit). */
  label: string;
  /** Distance from the strip origin to the chip's left edge. */
  x: number;
  width: number;
  bg: string;
  border: string;
  fg: string;
  /** Width reserved for the kind glyph drawn before the text. */
  glyphWidth: number;
  /** Text offset inside the chip. */
  textX: number;
}

export interface BadgeStrip {
  chips: BadgeChip[];
  /** Total occupied width, first chip's left edge to last chip's right edge. */
  width: number;
  /** How many badges were dropped in favour of the "+N" chip. */
  dropped: number;
  height: number;
  radius: number;
  padX: number;
  font: string;
}

export interface BadgeLayoutOptions {
  theme: GraphTheme;
  /** The commit's lane colour; branch/remote chips are tinted with it. */
  laneColor: string;
  chipHeight: number;
  padX: number;
  gap: number;
  /** The strip is clipped to this width; the remainder collapses into "+N". */
  maxWidth: number;
  /** One chip never exceeds this width (labels ellipsize). */
  maxChipWidth: number;
  font?: string;
  /** Copy for the overflow chip ("+3"); injected so this module stays copy-free. */
  overflowLabel: (dropped: number) => string;
}

const GLYPH_WIDTH = 9;

export function layoutBadgeStrip(
  badges: readonly RefBadge[],
  options: BadgeLayoutOptions,
): BadgeStrip {
  const ctx = measureContext();
  const font = options.font ?? `${options.theme.fontSizeXs}px ${options.theme.fontFamily}`;
  ctx.font = font;

  const colorFor = badgePalette(options);
  const chips: BadgeChip[] = [];
  let cursor = 0;

  for (const badge of badges) {
    const colors = colorFor(badge.kind);
    const glyphWidth = badge.kind === "other" ? 0 : GLYPH_WIDTH;
    const textBudget = options.maxChipWidth - glyphWidth - options.padX * 2;
    const label = ellipsize(ctx, badge.label, textBudget);
    const width = Math.round(options.padX * 2 + glyphWidth + ctx.measureText(label).width);
    chips.push({
      badge,
      label,
      x: cursor,
      width,
      glyphWidth,
      textX: options.padX + glyphWidth,
      ...colors,
    });
    cursor += width + options.gap;
  }

  let dropped = 0;
  let width = chips.length > 0 ? cursor - options.gap : 0;

  while (chips.length > 0 && width > options.maxWidth) {
    chips.pop();
    dropped += 1;
    const last = chips[chips.length - 1];
    width = last ? last.x + last.width : 0;
  }

  if (dropped > 0) {
    const label = options.overflowLabel(dropped);
    const width2 = Math.round(options.padX * 2 + ctx.measureText(label).width);
    const x = chips.length > 0 ? width + options.gap : 0;
    chips.push({
      badge: { id: "overflow", kind: "other", label },
      label,
      x,
      width: width2,
      glyphWidth: 0,
      textX: options.padX,
      ...colorFor("other"),
    });
    width = x + width2;
  }

  return {
    chips,
    width,
    dropped,
    height: options.chipHeight,
    radius: options.theme.radiusSm,
    padX: options.padX,
    font,
  };
}

/** Index of the chip under `x` (strip-local), or -1. */
export function hitTestStrip(strip: BadgeStrip, x: number): number {
  for (let i = 0; i < strip.chips.length; i += 1) {
    const chip = strip.chips[i];
    if (x >= chip.x && x <= chip.x + chip.width) return i;
  }
  return -1;
}

export function ellipsize(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string {
  if (maxWidth <= 0) return "";
  if (ctx.measureText(text).width <= maxWidth) return text;
  const suffix = "…";
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (ctx.measureText(text.slice(0, mid) + suffix).width <= maxWidth) lo = mid;
    else hi = mid - 1;
  }
  return lo === 0 ? suffix : text.slice(0, lo) + suffix;
}

function badgePalette(
  options: BadgeLayoutOptions,
): (kind: RefKind) => { bg: string; border: string; fg: string } {
  const theme = options.theme;
  return (kind) => {
    switch (kind) {
      case "head":
        return { bg: theme.accent, border: theme.accent, fg: theme.accentContrast };
      case "branch":
        return {
          bg: alpha(options.laneColor, 0.16),
          border: alpha(options.laneColor, 0.55),
          fg: options.laneColor,
        };
      case "remote":
        return {
          bg: alpha(theme.badgeBg, 0.7),
          border: alpha(options.laneColor, 0.4),
          fg: theme.fgMuted,
        };
      case "tag":
        return { bg: theme.badgeBg, border: alpha(theme.badgeFg, 0.35), fg: theme.badgeFg };
      default:
        return { bg: theme.badgeBg, border: alpha(theme.badgeFg, 0.25), fg: theme.fgMuted };
    }
  };
}
