/**
 * Diff wire + view contracts (docs/03 `wc_diff_load` / `wc_diff_hunks_load`,
 * docs/02 §4 `domain/diff.rs`).
 *
 * The Rust side owns these shapes; this file is the TypeScript mirror the diff
 * components are written against, kept camelCase because every Rust struct in
 * the command layer carries `#[serde(rename_all = "camelCase")]`. When
 * `src-tauri/src/domain/diff.rs` lands, reconcile field names here — this file
 * is the only place the diff module's field names are spelled.
 */

import type { RepoId, StatusDelta } from "../../bridges/bindings";

export type { RepoId, StatusDelta };

/** The six perspectives the engine classifies diffs by (docs/00 §13–14). */
export type DiffPerspective =
  | "indexToBase"
  | "mineToMergeBase"
  | "mineToTheirs"
  | "theirsToMergeBase"
  | "workingTreeToBase"
  | "workingTreeToIndex";

/** Options accepted by `wc_diff_load` / `history_diff_document_load`. */
export interface DiffOptions {
  /** Context lines around each hunk (the `Context:` menu, evidence nib
   *  `GTDiffContextLinesMenuController`). */
  contextLines?: number;
  /** Git's `-w`: recompute the diff ignoring whitespace-only changes. */
  ignoreWhitespace?: boolean;
  /** `toggleShowsFullDiff:` — render the whole file, not only hunks. */
  showFull?: boolean;
  /** `ColorSyntaxInDiffs` — backend may pre-tokenize; the renderer tolerates
   *  plain text and would colour tokens when the Shiki theme lands. */
  syntax?: boolean;
}

/** What the detail pane is showing. Mirrors the `<DiffView>` states in
 *  docs/04 §6 (`loading` / `error` are load states, not document kinds). */
export type DiffKind = "text" | "binary" | "image" | "empty";

export type DiffRowKind = "context" | "added" | "removed" | "noNewline";

/** One line of a hunk. `text` never carries the leading `+`/`-`/space marker;
 *  `oldLine`/`newLine` are 1-based or null on the side the line does not exist. */
export interface DiffRow {
  kind: DiffRowKind;
  text: string;
  oldLine: number | null;
  newLine: number | null;
}

/** The identity of a hunk for `wc_chunk_apply` / `wc_chunk_discard`
 *  (docs/02 §13: "the same shape the selectors imply (EV:sel/stageChunk)"). */
export interface HunkRef {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  /** Trailing `@@ … @@` section text, when the backend parsed one. */
  section?: string | null;
}

export interface DiffHunk extends HunkRef {
  /** Display header as the backend rendered it, e.g. `@@ -1,5 +1,6 @@ fn main()`. */
  header: string;
  rows: DiffRow[];
}

export interface DiffStats {
  additions: number;
  deletions: number;
  hunks: number;
  lines: number;
}

/** Summary for `kind: "binary"` documents — sizes are the only content the
 *  renderer can honestly show for a binary pair. */
export interface BinarySummary {
  oldSize: number | null;
  newSize: number | null;
  oldOid: string | null;
  newOid: string | null;
  /** True when both sides hash equal (a mode-only change, e.g. 100644→100755). */
  identical: boolean;
}

/** Set when the backend capped document generation. The UI must surface this —
 *  it is the "never silently truncate" contract on the Rust side. */
export interface DiffElision {
  reason: "large" | "timeout" | "config";
  hiddenHunks: number;
  hiddenLines: number;
  limitBytes?: number | null;
  /** Backend-supplied explanation, rendered verbatim when present. */
  message?: string | null;
}

export interface FileModeChange {
  /** Octal string as git prints it, e.g. "100644". */
  oldMode: string | null;
  newMode: string | null;
}

/** `wc_diff_load` / `history_diff_document_load` return value. */
export interface DiffDocument {
  path: string;
  /** Set when the diff is a rename (old name shown as `old → new`). */
  oldPath: string | null;
  kind: DiffKind;
  perspective: DiffPerspective;
  /** Shiki language id when the backend could infer one; null for unknown. */
  language: string | null;
  mode: FileModeChange | null;
  binary: BinarySummary | null;
  hunks: DiffHunk[];
  stats: DiffStats;
  elided: DiffElision | null;
}

/** `wc_chunk_apply` direction (docs/03 §3). */
export type ChunkDirection = "stage" | "unstage";

export type ImageSide = "old" | "new";

/** `wc_image_data_load` return value. */
export interface ImagePayload {
  mime: string;
  /** Base64 of the blob; the renderer builds a `data:` URL. */
  base64: string;
}

/** Dialog copy for the large-diff gate — our text (docs/04 §10). */
export interface DiffGateCopy {
  title: string;
  body: string;
}
