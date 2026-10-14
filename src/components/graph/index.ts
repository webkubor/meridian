/**
 * The graph module's public surface (docs/04 §2.5, docs/02 §13).
 *
 * `CommitGraphCell` draws one row; everything else is the model the commit list
 * (src/components/commit-list) and the history pane build on:
 *   - `GraphLayout`/`GraphLaneSegment` — the contract with `history_graph_compute`.
 *   - `buildSegmentIndex`/`segmentIdsAt` — per-row stroke lookup for virtualisation.
 *   - `parseDecorations` + `layoutBadgeStrip` — refs → measured badge chips.
 *   - `useGraphTheme` — tokens (incl. `.theme-dark`) read for canvas.
 */

export { default as CommitGraphCell } from "./CommitGraphCell.vue";

export {
  buildSegmentIndex,
  continuationLanes,
  gutterWidth,
  laneCountOf,
  laneX,
  metricsFor,
  segmentIdsAt,
  CELL_ROWS,
  LANE_PAD,
  type GraphMetrics,
  type SegmentIndex,
} from "./geometry";

export { hitTestStrip, layoutBadgeStrip, type BadgeChip, type BadgeStrip, type BadgeLayoutOptions } from "./badges";

export { hasHeadDecoration, parseDecorations, type ParseDecorationsOptions, type RefBadge, type RefKind } from "./refs";

export { alpha, laneColor, readGraphTheme, useGraphTheme, type GraphTheme } from "./theme";

export { paintRow, type PaintInput } from "./renderer";

export type {
  GraphDensity,
  GraphEdgeKind,
  GraphEdgeSide,
  GraphLaneSegment,
  GraphLayout,
} from "./types";
