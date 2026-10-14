/**
 * The renderer's view of the history engine's lane assignment.
 *
 * docs/02 §13 and docs/03 §5 place the graph *computation* in Rust
 * (`engine::history::draw_graph`, exposed as `history_graph_compute` →
 * `GraphLayout { lanes, segments }`); this module owns the *drawing*. The
 * shapes below are the contract we expect from that command. They are declared
 * here — in the renderer's own tree — so this module compiles and runs before
 * the command lands, exactly like docs/04 §2.5 describes it.
 *
 * Cross-module expectation (for the engine / history-pane authors):
 *   - `history_graph_compute({ repo, oids })` returns `GraphLayout` where the
 *     `lanes`/`segments` row indices are positions in the `oids` array it was
 *     given (the same order `history_commits_load` returned them, so they line
 *     up with the `commits` array the UI holds).
 *   - Row indices in `segments` use the same row space as `lanes`.
 *   - Lane indices start at 0; the colour is `palette[lane % 9]` (docs/04 §2.5,
 *     the evidence's modulo-9 cycle).
 */

/** How a lane stroke attaches at its ends. Mirrors `GTGitCommitGraphLane.starting/.ending`. */
export type GraphEdgeKind = "straight" | "merge" | "fork" | "bend";

/**
 * One stroke of the graph, connecting the grid point (fromRow, fromLane) to
 * (toRow, toLane) in row/lane space.
 *
 * A vertical run between two commits is a `straight` segment with
 * `fromRow < toRow` and both lanes equal. A merge/fork that changes lanes is a
 * `bend` (or `merge`/`fork`, drawn identically) whose two lane fields differ —
 * the renderer draws a smooth S-curve between the two grid points.
 *
 * Interop: `src/components/contracts.ts` (the UI layer's shared view model)
 * spells the same stroke as `{ lane, toLane, fromRow, toRow, kind }`, where
 * `lane` is the *start* lane. That is this shape with `fromLane: lane`; the
 * renderer accepts either (`fromLane` defaults to `lane`).
 */
export interface GraphLaneSegment {
  /** Layout row (index into the commit array) where the stroke begins. */
  fromRow: number;
  /** Layout row where the stroke ends. `toRow >= fromRow`. */
  toRow: number;
  /**
   * The stroke's colour lane: `palette[lane % 9]` (docs/04 §2.5). For strokes
   * that change lanes this is the lane the stroke is owned by (the child line
   * that spawned it) — the engine picks; the renderer only reads it.
   */
  lane: number;
  /** How the stroke attaches at its ends (semantic; the geometry comes from the lane fields). */
  kind: GraphEdgeKind;
  /** Lane at the `fromRow` end. Defaults to `lane`. */
  fromLane?: number;
  /** Lane at the `toRow` end. Defaults to `lane`. */
  toLane?: number;
}

/** The engine's answer for a loaded page of commits. */
export interface GraphLayout {
  /**
   * Bubble lane per layout row, parallel to the `commits` array the engine was
   * asked about. `-1` marks a row with no commit. Optional: when the engine (or
   * the shared `contracts.ts` shape) reports bubble lanes separately — one
   * `{ lane }` per commit row — pass them to the list's `graphRows` prop
   * instead.
   */
  lanes?: number[];
  /** Every stroke needed to draw the page. */
  segments: GraphLaneSegment[];
  /** Highest lane index used + 1. Optional; derived from `lanes`/`segments` when absent. */
  laneCount?: number;
}

/** Row density (docs/04 §2.3/§2.5): regular = 14 px lanes, compact = 10 px lanes. */
export type GraphDensity = "compact" | "regular";

/** Which side of a truncated graph edge a continuation marker belongs to. */
export type GraphEdgeSide = "top" | "bottom";
