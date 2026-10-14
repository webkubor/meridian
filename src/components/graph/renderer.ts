/**
 * Canvas renderer for one commit row's graph cell (docs/04 §2.5).
 *
 * The component layer owns pixels-per-row; this module owns marks: lane
 * strokes (straight runs and merge/fork S-curves), continuation nibs for
 * truncated page edges, the commit bubble with its head decoration, the
 * ref-tip notch, and the ref badge chips. All of it is drawn from the model the
 * list hands down — no layout state lives in the canvas.
 *
 * Every mark is drawn in *content-space* y (the row canvas is translated by
 * `-rowTop`), so a stroke that spans rows is identical in each row's canvas and
 * the browser's own clip seams the pieces together.
 */

import type { BadgeStrip } from "./badges";
import { laneX, type GraphMetrics } from "./geometry";
import { laneColor, type GraphTheme } from "./theme";
import type { GraphLaneSegment, GraphEdgeSide } from "./types";

export interface PaintInput {
  /** Row rect in CSS pixels (the canvas' own size). */
  width: number;
  height: number;
  /** Content-space y of the row's top edge. */
  rowTop: number;
  rowHeight: number;
  /** This row's layout row index, or null for display rows without a commit. */
  commitRow: number | null;
  /** This row's bubble lane (`-1` when absent). */
  lane: number | null;
  /** Strokes crossing or terminating at this row. */
  segments: readonly GraphLaneSegment[];
  /** Content-space centre y of every layout row (index = layout row). */
  centers: Float64Array;
  /** Measured badge strip, or null when the commit carries no refs. */
  strip: BadgeStrip | null;
  /** x where the badge strip starts (the gutter width). */
  stripOriginX: number;
  hoveredChip: number;
  /** Lanes whose strokes continue past the loaded page, at this row's edge. */
  continuation: { top: readonly number[]; bottom: readonly number[] };
  metrics: GraphMetrics;
  theme: GraphTheme;
  selected: boolean;
  hovered: boolean;
  isHead: boolean;
}

export function paintRow(ctx: CanvasRenderingContext2D, input: PaintInput): void {
  ctx.clearRect(0, 0, input.width, input.height);

  ctx.save();
  ctx.translate(0, -input.rowTop);

  // 1. Lane strokes under everything else.
  for (const segment of input.segments) {
    paintSegment(ctx, segment, input);
  }

  // 2. Continuation nibs (the graph is truncated here — top or bottom page edge).
  const centerY = input.rowTop + input.rowHeight / 2;
  for (const lane of input.continuation.top) {
    paintNib(ctx, laneX(lane, input.metrics), input.rowTop + 7, "top", laneColor(input.theme, lane));
  }
  for (const lane of input.continuation.bottom) {
    paintNib(
      ctx,
      laneX(lane, input.metrics),
      input.rowTop + input.rowHeight - 7,
      "bottom",
      laneColor(input.theme, lane),
    );
  }

  // 3. The commit bubble.
  if (input.commitRow != null && input.lane != null && input.lane >= 0) {
    paintBubble(ctx, laneX(input.lane, input.metrics), centerY, input);
  }

  ctx.restore();

  // 4. Ref badges — screen space, after the strip anchor (never scroll with the graph).
  if (input.strip && input.strip.chips.length > 0) {
    paintChips(ctx, input.strip, input);
  }
}

function paintSegment(
  ctx: CanvasRenderingContext2D,
  segment: GraphLaneSegment,
  input: PaintInput,
): void {
  const { metrics, theme } = input;
  const fromLane = segment.fromLane ?? segment.lane;
  const toLane = segment.toLane ?? segment.lane;
  const x0 = laneX(fromLane, metrics);
  const x1 = laneX(toLane, metrics);
  const y0 = yOfRow(input, segment.fromRow, input.rowTop);
  const y1 = yOfRow(input, segment.toRow, input.rowTop + input.rowHeight);

  ctx.strokeStyle = laneColor(theme, segment.lane);
  ctx.lineWidth = metrics.strokeWidth;
  ctx.lineCap = "round";
  ctx.beginPath();
  ctx.moveTo(x0, y0);
  if (x0 === x1) {
    ctx.lineTo(x1, y1);
  } else {
    // Smooth S: vertical tangents at both ends, bend clamped so a 1-row hop
    // still reads as a curve while a long diagonal stays mostly vertical.
    const bend = Math.min(Math.abs(y1 - y0) * 0.5, metrics.laneWidth * 1.4);
    ctx.bezierCurveTo(x0, y0 + bend, x1, y1 - bend, x1, y1);
  }
  ctx.stroke();
}

function paintNib(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  side: GraphEdgeSide,
  color: string,
): void {
  const arm = side === "bottom" ? -4.5 : 4.5;
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  ctx.lineCap = "round";
  ctx.beginPath();
  ctx.moveTo(x - 3.5, y + arm);
  ctx.lineTo(x, y);
  ctx.lineTo(x + 3.5, y + arm);
  ctx.stroke();
}

function paintBubble(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  input: PaintInput,
): void {
  const { metrics, theme } = input;

  // Ref-tip notch: this commit carries refs (a branch head, tag, …).
  if (input.strip && input.strip.chips.length > 0 && !input.isHead) {
    const color = laneColor(theme, input.lane ?? 0);
    const radius = metrics.bubbleRadius;
    ctx.beginPath();
    ctx.moveTo(x - 3, y - radius - 6);
    ctx.lineTo(x + 3, y - radius - 6);
    ctx.lineTo(x, y - radius - 1);
    ctx.closePath();
    ctx.fillStyle = color;
    ctx.fill();
  }

  const radius = metrics.bubbleRadius;
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.fillStyle = theme.bubbleBg;
  ctx.fill();
  ctx.lineWidth = 1;
  ctx.strokeStyle = input.selected || input.hovered ? theme.accent : theme.bubbleBorder;
  ctx.stroke();

  if (input.isHead) {
    // Head decoration: the commit HEAD resolves to (docs/04 §2.5 — bubble + ring).
    ctx.beginPath();
    ctx.arc(x, y, radius + 2.5, 0, Math.PI * 2);
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = theme.accent;
    ctx.stroke();
  }

  if (input.hovered && !input.selected) {
    ctx.beginPath();
    ctx.arc(x, y, radius + 3.5, 0, Math.PI * 2);
    ctx.lineWidth = 1;
    ctx.strokeStyle = alphaStroke(theme);
    ctx.stroke();
  }
}

function paintChips(
  ctx: CanvasRenderingContext2D,
  strip: BadgeStrip,
  input: PaintInput,
): void {
  const y = (input.rowHeight - strip.height) / 2;
  ctx.save();
  ctx.beginPath();
  ctx.rect(0, 0, input.width, input.rowHeight);
  ctx.clip();
  ctx.font = strip.font;
  ctx.textBaseline = "middle";

  strip.chips.forEach((chip, index) => {
    const x = input.stripOriginX + chip.x;
    const hovered = index === input.hoveredChip;

    roundedRect(ctx, x, y, chip.width, strip.height, strip.radius);
    ctx.fillStyle = chip.bg;
    ctx.fill();
    ctx.lineWidth = 1;
    ctx.strokeStyle = hovered ? input.theme.accent : chip.border;
    ctx.stroke();

    const textX = x + chip.textX;
    if (chip.glyphWidth > 0) {
      paintGlyph(ctx, chip.badge.kind, textX - chip.glyphWidth / 2 - 1.5, y + strip.height / 2, chip.fg);
    }
    ctx.fillStyle = chip.fg;
    ctx.fillText(chip.label, textX, y + strip.height / 2 + 0.5);
  });

  ctx.restore();
}

function paintGlyph(
  ctx: CanvasRenderingContext2D,
  kind: "head" | "branch" | "remote" | "tag" | "other",
  x: number,
  y: number,
  color: string,
): void {
  ctx.strokeStyle = color;
  ctx.fillStyle = color;
  ctx.lineWidth = 1.2;
  switch (kind) {
    case "head":
    case "branch": {
      ctx.beginPath();
      ctx.arc(x, y, 2.6, 0, Math.PI * 2);
      ctx.fill();
      break;
    }
    case "remote": {
      ctx.beginPath();
      ctx.arc(x, y, 2.6, 0, Math.PI * 2);
      ctx.stroke();
      break;
    }
    case "tag": {
      ctx.beginPath();
      ctx.moveTo(x - 3.2, y - 3.2);
      ctx.lineTo(x + 1.2, y - 3.2);
      ctx.lineTo(x + 3.4, y);
      ctx.lineTo(x + 1.2, y + 3.2);
      ctx.lineTo(x - 3.2, y + 3.2);
      ctx.closePath();
      ctx.stroke();
      break;
    }
    default:
      break;
  }
}

/**
 * Content-space y of a layout row's centre. The context is already translated
 * by `-rowTop` (so every row's canvas draws the same content-space geometry),
 * which is why the value is *not* made row-local here.
 */
function yOfRow(input: PaintInput, row: number, fallback: number): number {
  const value = input.centers[row];
  return Number.isFinite(value) ? value : fallback;
}

function alphaStroke(theme: GraphTheme): string {
  // The hover ring is a thinner, quieter sibling of the selection ring.
  return theme.fgMuted;
}

function roundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
): void {
  const r = Math.max(0, Math.min(radius, width / 2, height / 2));
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + width - r, y);
  ctx.arcTo(x + width, y, x + width, y + r, r);
  ctx.lineTo(x + width, y + height - r);
  ctx.arcTo(x + width, y + height, x + width - r, y + height, r);
  ctx.lineTo(x + r, y + height);
  ctx.arcTo(x, y + height, x, y + height - r, r);
  ctx.lineTo(x, y + r);
  ctx.arcTo(x, y, x + r, y, r);
  ctx.closePath();
}
