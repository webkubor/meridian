/**
 * Graph geometry: lane/lane-width math, gutter sizing, and the per-row segment
 * index that makes virtualised drawing cheap.
 *
 * Coordinate model. Rows are layout rows (indices into `GraphLayout.lanes`).
 * Vertical positions are supplied by the *list*, which knows the display row
 * offsets (date groups make row heights non-uniform): `centers[row]` is the
 * content-space y of row `row`'s centre. Horizontal positions are lane-local
 * and identical for every row, so the text column stays aligned.
 */

import type { GraphDensity, GraphEdgeSide, GraphLaneSegment, GraphLayout } from "./types";
import type { GraphTheme } from "./theme";

export interface GraphMetrics {
  density: GraphDensity;
  laneWidth: number;
  /** Horizontal padding before lane 0 and after the last lane. */
  lanePad: number;
  strokeWidth: number;
  bubbleRadius: number;
}

/** Horizontal padding inside the gutter, before lane 0. */
export const LANE_PAD = 6;

/** Segment spans longer than this are bucketed by `CELL_ROWS`-row cells. */
export const CELL_ROWS = 64;

export function metricsFor(density: GraphDensity, theme: GraphTheme): GraphMetrics {
  return {
    density,
    laneWidth: density === "compact" ? theme.laneWidthCompact : theme.laneWidth,
    lanePad: LANE_PAD,
    strokeWidth: density === "compact" ? 1.5 : 2,
    bubbleRadius: theme.bubbleRadius,
  };
}

/** x of a lane's centre line, in gutter-local coordinates. */
export function laneX(lane: number, metrics: GraphMetrics): number {
  return metrics.lanePad + lane * metrics.laneWidth + metrics.laneWidth / 2;
}

/** Highest lane index used by a layout, + 1. */
export function laneCountOf(layout: GraphLayout | null, lanes?: readonly number[]): number {
  if (!layout) return 0;
  let max = -1;
  for (const lane of layout.lanes ?? lanes ?? []) {
    if (lane > max) max = lane;
  }
  for (const segment of layout.segments) {
    if (segment.lane > max) max = segment.lane;
    if (segment.fromLane != null && segment.fromLane > max) max = segment.fromLane;
    if (segment.toLane != null && segment.toLane > max) max = segment.toLane;
  }
  return Math.max(layout.laneCount ?? 0, max + 1, 0);
}

/** Width of the graph gutter for a lane count. */
export function gutterWidth(laneCount: number, metrics: GraphMetrics): number {
  if (laneCount <= 0) return 0;
  return laneX(laneCount - 1, metrics) + metrics.laneWidth / 2 + metrics.lanePad;
}

/**
 * Per-row lookup of the strokes that cross a row.
 *
 * Short spans (≤ 2·CELL_ROWS) are listed per row; long spans are bucketed into
 * `CELL_ROWS`-row cells and filtered on lookup, so memory stays O(rows/CELL +
 * segments) instead of O(Σ span) — the paging case (hundreds of thousands of
 * rows, a handful of long lanes) is what this protects.
 */
export interface SegmentIndex {
  segments: readonly GraphLaneSegment[];
  rows: Array<number[] | undefined>;
  cells: Map<number, number[]>;
}

/**
 * Build the index for `segments` over `rowCount` layout rows. Row indices are
 * clamped to the loaded window; segments entirely outside it are dropped.
 */
export function buildSegmentIndex(
  segments: readonly GraphLaneSegment[],
  rowCount: number,
): SegmentIndex {
  const rows: Array<number[] | undefined> = new Array(rowCount);
  const cells = new Map<number, number[]>();
  if (rowCount <= 0) return { segments, rows, cells };

  segments.forEach((segment, id) => {
    const a = Math.max(0, Math.min(segment.fromRow, segment.toRow));
    const b = Math.min(rowCount - 1, Math.max(segment.fromRow, segment.toRow));
    if (b < a) return;
    if (b - a <= CELL_ROWS * 2) {
      for (let row = a; row <= b; row += 1) {
        (rows[row] ??= []).push(id);
      }
    } else {
      for (let cell = Math.floor(a / CELL_ROWS); cell <= Math.floor(b / CELL_ROWS); cell += 1) {
        let list = cells.get(cell);
        if (!list) {
          list = [];
          cells.set(cell, list);
        }
        list.push(id);
      }
    }
  });

  return { segments, rows, cells };
}

/** Ids of the segments whose span contains `row` (clipped to the loaded window). */
export function segmentIdsAt(index: SegmentIndex, row: number): readonly number[] {
  const short = index.rows[row];
  const cell = index.cells.get(Math.floor(row / CELL_ROWS));
  if (!cell) return short ?? EMPTY_IDS;
  const out: number[] = short ? [...short] : [];
  for (const id of cell) {
    const segment = index.segments[id];
    if (segment && segment.fromRow <= row && row <= segment.toRow) out.push(id);
  }
  return out;
}

const EMPTY_IDS: readonly number[] = [];

/**
 * Lanes that must show a continuation marker ("the graph continues past the
 * loaded page") at `row`, on `side`.
 *
 * Bottom: every run that stops at this row but is not this row's commit line,
 * plus the commit's own lane (its parents are not loaded — only asked for when
 * the page is truncated). Top: every run that starts at this row but does not
 * belong to this row's commit (the page began mid-graph, e.g. compare mode).
 */
export function continuationLanes(
  layout: GraphLayout,
  row: number,
  side: GraphEdgeSide,
  ownLane: number | null = layout.lanes?.[row] ?? null,
): number[] {
  const lanes = new Set<number>();
  for (const segment of layout.segments) {
    if (side === "bottom" && segment.toRow === row) {
      const end = segment.toLane ?? segment.lane;
      if (end !== ownLane) lanes.add(end);
    }
    if (side === "top" && segment.fromRow === row) {
      const start = segment.fromLane ?? segment.lane;
      if (start !== ownLane) lanes.add(start);
    }
  }
  if (side === "bottom" && ownLane != null && ownLane >= 0) lanes.add(ownLane);
  return [...lanes];
}
