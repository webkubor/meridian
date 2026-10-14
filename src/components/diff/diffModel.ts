/**
 * The diff view model: pure functions from a `DiffDocument` to the rows both
 * renderers draw. Nothing here touches Vue or Tauri, so the model can be
 * exercised directly (and it is: see the module's sanity script in the return
 * notes).
 *
 * Responsibilities:
 *  - intra-line (word) highlighting by token-LCS over paired −/+ lines;
 *  - split-view pairing of removed/added runs;
 *  - gap accounting between hunks (how many context lines are hidden);
 *  - render-limit slicing that reports exactly what it elided.
 */

import type {
  DiffDocument,
  DiffHunk,
  DiffRow,
  DiffRowKind,
  HunkRef,
} from "./types";

export const RENDER_LINE_CAP = 2000;

export const CONTEXT_PRESETS = [1, 3, 5, 10, 25] as const;

export interface WordSpan {
  start: number;
  end: number;
}

export interface DiffSegment {
  text: string;
  highlight: boolean;
}

export interface DiffLine {
  kind: DiffRowKind;
  text: string;
  oldLine: number | null;
  newLine: number | null;
  segments: DiffSegment[];
}

export interface SplitRow {
  left: DiffLine | null;
  right: DiffLine | null;
}

export interface DiffHunkView {
  ref: HunkRef;
  header: string;
  id: string;
  lines: DiffLine[];
  splitRows: SplitRow[];
  additions: number;
  deletions: number;
  /** Context lines hidden between the previous gap and this hunk; null when
   *  the hunks are adjacent or the count is not derivable from the header. */
  gapBefore: number | null;
  /** True when there is a hidden gap whose size cannot be stated from the
   *  hunk headers alone — the affordance is shown without a number rather
   *  than with a wrong one. */
  gapBeforeUnknown: boolean;
  /** This hunk's tail was cut by the render limit. */
  clipped: boolean;
}

export interface DiffViewModel {
  hunks: DiffHunkView[];
  shownLines: number;
  totalLines: number;
  hiddenLines: number;
  hiddenHunks: number;
  truncated: boolean;
  /** Offer a "show more context below the last hunk" affordance. */
  tailGap: boolean;
  hunkCount: number;
  additions: number;
  deletions: number;
}

export interface DiffViewModelOptions {
  /** Compute intra-line spans (default true). */
  wordDiff?: boolean;
  /** Maximum rendered lines; null renders everything. */
  maxLines?: number | null;
  /** The document was requested with `showFull` — suppress the tail affordance. */
  fullFile?: boolean;
}

// --- Word-level diffing ------------------------------------------------------

interface Token {
  text: string;
  start: number;
  end: number;
}

/** Words, whitespace, and symbol runs — script-agnostic by unicode property. */
const TOKEN_PATTERN = /[\p{L}\p{N}_]+|\s+|[^\s\p{L}\p{N}_]+/gu;

/** Above this many tokens per side the LCS table is skipped and the whole
 *  changed region is highlighted instead — the line-level colour still shows
 *  the change, nothing is hidden. */
const LCS_TOKEN_CAP = 600;

export function tokenizeLine(text: string): Token[] {
  const tokens: Token[] = [];
  for (const match of text.matchAll(TOKEN_PATTERN)) {
    const start = match.index ?? 0;
    tokens.push({ text: match[0], start, end: start + match[0].length });
  }
  return tokens;
}

function spansFromTokens(text: string, tokens: Token[], indexes: number[]): WordSpan[] {
  const spans: WordSpan[] = [];
  for (const index of indexes) {
    const token = tokens[index];
    if (!token) continue;
    const last = spans[spans.length - 1];
    if (last && token.start <= last.end) last.end = Math.max(last.end, token.end);
    else spans.push({ start: token.start, end: token.end });
  }
  const trimmed: WordSpan[] = [];
  for (const span of spans) {
    const body = text.slice(span.start, span.end);
    const leading = body.length - body.trimStart().length;
    const trailing = body.length - body.trimEnd().length;
    const start = span.start + leading;
    const end = span.end - trailing;
    if (end > start) trimmed.push({ start, end });
  }
  return trimmed;
}

/**
 * Token-level LCS between two lines, returned as highlight spans on each side
 * in full-line coordinates. Identical prefix/suffix tokens are trimmed first,
 * which is what keeps cost linear on the common "one edit in a long line" case.
 */
export function wordDiffSpans(
  before: string,
  after: string,
): { before: WordSpan[]; after: WordSpan[] } {
  const a = tokenizeLine(before);
  const b = tokenizeLine(after);

  let prefix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix]!.text === b[prefix]!.text) prefix++;
  let suffix = 0;
  while (
    suffix < a.length - prefix &&
    suffix < b.length - prefix &&
    a[a.length - 1 - suffix]!.text === b[b.length - 1 - suffix]!.text
  ) {
    suffix++;
  }

  const aMid = a.slice(prefix, a.length - suffix);
  const bMid = b.slice(prefix, b.length - suffix);
  if (aMid.length === 0 && bMid.length === 0) return { before: [], after: [] };
  if (aMid.length > LCS_TOKEN_CAP || bMid.length > LCS_TOKEN_CAP) {
    return {
      before: spansFromTokens(before, aMid, aMid.map((_, index) => index)),
      after: spansFromTokens(after, bMid, bMid.map((_, index) => index)),
    };
  }

  const n = aMid.length;
  const m = bMid.length;
  const width = m + 1;
  // dp[i][j] = LCS length of aMid[i..] and bMid[j..]
  const dp = new Uint32Array((n + 1) * width);
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i * width + j] =
        aMid[i]!.text === bMid[j]!.text
          ? dp[(i + 1) * width + j + 1]! + 1
          : Math.max(dp[(i + 1) * width + j]!, dp[i * width + j + 1]!);
    }
  }

  const beforeChanged: number[] = [];
  const afterChanged: number[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (aMid[i]!.text === bMid[j]!.text) {
      i++;
      j++;
    } else if (dp[(i + 1) * width + j]! >= dp[i * width + j + 1]!) {
      beforeChanged.push(i);
      i++;
    } else {
      afterChanged.push(j);
      j++;
    }
  }
  while (i < n) beforeChanged.push(i++);
  while (j < m) afterChanged.push(j++);

  return {
    before: spansFromTokens(before, aMid, beforeChanged),
    after: spansFromTokens(after, bMid, afterChanged),
  };
}

export function segmentsFor(text: string, spans: WordSpan[]): DiffSegment[] {
  if (spans.length === 0 || text.length === 0) return [{ text, highlight: false }];
  const segments: DiffSegment[] = [];
  let cursor = 0;
  for (const span of spans) {
    if (span.start > cursor) segments.push({ text: text.slice(cursor, span.start), highlight: false });
    segments.push({ text: text.slice(span.start, span.end), highlight: true });
    cursor = span.end;
  }
  if (cursor < text.length) segments.push({ text: text.slice(cursor), highlight: false });
  return segments.length > 0 ? segments : [{ text, highlight: false }];
}

// --- Hunk view ---------------------------------------------------------------

export function hunkId(hunk: Pick<HunkRef, "oldStart" | "oldLines" | "newStart" | "newLines">): string {
  return `${hunk.oldStart},${hunk.oldLines}:${hunk.newStart},${hunk.newLines}`;
}

export function hunkRefOf(hunk: HunkRef): HunkRef {
  const ref: HunkRef = {
    oldStart: hunk.oldStart,
    oldLines: hunk.oldLines,
    newStart: hunk.newStart,
    newLines: hunk.newLines,
  };
  if (hunk.section != null) ref.section = hunk.section;
  return ref;
}

export function formatHunkHeader(hunk: HunkRef): string {
  const section = hunk.section ? ` ${hunk.section}` : "";
  return `@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@${section}`;
}

function diffLine(row: DiffRow, segments: DiffSegment[]): DiffLine {
  return {
    kind: row.kind,
    text: row.text,
    oldLine: row.oldLine,
    newLine: row.newLine,
    segments,
  };
}

function plainLine(row: DiffRow): DiffLine {
  return diffLine(row, segmentsFor(row.text, []));
}

/** Pair a removed run with the following added run by index and compute the
 *  intra-line spans for each pair (the pairs the word diff can honestly speak
 *  about — unpaired lines keep block colour only). */
function pairedSegments(
  removedRun: DiffRow[],
  addedRun: DiffRow[],
  wordDiff: boolean,
): { removed: DiffSegment[][]; added: DiffSegment[][] } {
  const pairs = wordDiff ? Math.min(removedRun.length, addedRun.length) : 0;
  const spans: { before: WordSpan[]; after: WordSpan[] }[] = [];
  for (let k = 0; k < pairs; k++) {
    spans.push(wordDiffSpans(removedRun[k]!.text, addedRun[k]!.text));
  }
  const removed = removedRun.map((row, k) => segmentsFor(row.text, k < pairs ? spans[k]!.before : []));
  const added = addedRun.map((row, k) => segmentsFor(row.text, k < pairs ? spans[k]!.after : []));
  return { removed, added };
}

export function buildHunkView(hunk: DiffHunk, wordDiff: boolean): DiffHunkView {
  const lines: DiffLine[] = [];
  const splitRows: SplitRow[] = [];
  let additions = 0;
  let deletions = 0;
  let index = 0;
  let lastKind: DiffRowKind = "context";

  while (index < hunk.rows.length) {
    const row = hunk.rows[index]!;

    if (row.kind === "removed") {
      const removedRun: DiffRow[] = [];
      while (index < hunk.rows.length && hunk.rows[index]!.kind === "removed") {
        removedRun.push(hunk.rows[index]!);
        index++;
      }
      const addedRun: DiffRow[] = [];
      while (index < hunk.rows.length && hunk.rows[index]!.kind === "added") {
        addedRun.push(hunk.rows[index]!);
        index++;
      }

      const segments = pairedSegments(removedRun, addedRun, wordDiff);
      const removedLines = removedRun.map((runRow, k) => diffLine(runRow, segments.removed[k]!));
      const addedLines = addedRun.map((runRow, k) => diffLine(runRow, segments.added[k]!));
      lines.push(...removedLines, ...addedLines);
      deletions += removedLines.length;
      additions += addedLines.length;

      const rows = Math.max(removedLines.length, addedLines.length);
      for (let k = 0; k < rows; k++) {
        splitRows.push({ left: removedLines[k] ?? null, right: addedLines[k] ?? null });
      }
      lastKind = addedLines.length > 0 ? "added" : "removed";
      continue;
    }

    if (row.kind === "added") {
      // An added run with no preceding removed run (pure insertion).
      const addedRun: DiffRow[] = [];
      while (index < hunk.rows.length && hunk.rows[index]!.kind === "added") {
        addedRun.push(hunk.rows[index]!);
        index++;
      }
      for (const runRow of addedRun) {
        const line = plainLine(runRow);
        lines.push(line);
        splitRows.push({ left: null, right: line });
      }
      additions += addedRun.length;
      lastKind = "added";
      continue;
    }

    const line = plainLine(row);
    lines.push(line);
    if (row.kind === "noNewline") {
      // The marker belongs to the side whose run it terminates.
      if (lastKind === "removed") splitRows.push({ left: line, right: null });
      else if (lastKind === "added") splitRows.push({ left: null, right: line });
      else splitRows.push({ left: line, right: line });
    } else {
      splitRows.push({ left: line, right: line });
      lastKind = "context";
    }
    index++;
  }

  return {
    ref: hunkRefOf(hunk),
    header: hunk.header || formatHunkHeader(hunk),
    id: hunkId(hunk),
    lines,
    splitRows,
    additions,
    deletions,
    gapBefore: null,
    gapBeforeUnknown: false,
    clipped: false,
  };
}

// --- Gap accounting ----------------------------------------------------------

/** How many old-side lines sit between the previous hunk and this one, when the
 *  hunk headers state it exactly. Null when they do not (zero-length hunks). */
function gapCount(prev: DiffHunk | null, next: DiffHunk): number | null {
  if (next.oldLines <= 0) return null;
  if (prev === null) return next.oldStart > 1 ? next.oldStart - 1 : null;
  if (prev.oldLines <= 0) return null;
  const prevEnd = prev.oldStart + prev.oldLines - 1;
  const hidden = next.oldStart - prevEnd - 1;
  return hidden > 0 ? hidden : null;
}

// --- Whole-view build --------------------------------------------------------

function sliceHunks(
  hunks: DiffHunkView[],
  maxLines: number | null,
): { hunks: DiffHunkView[]; hiddenLines: number; hiddenHunks: number } {
  if (maxLines === null) return { hunks, hiddenLines: 0, hiddenHunks: 0 };
  const kept: DiffHunkView[] = [];
  let used = 0;
  let hiddenLines = 0;
  let hiddenHunks = 0;
  for (const hunk of hunks) {
    if (hunk.lines.length === 0) {
      kept.push(hunk);
      continue;
    }
    if (used >= maxLines) {
      hiddenLines += hunk.lines.length;
      hiddenHunks += 1;
      continue;
    }
    const room = maxLines - used;
    if (hunk.lines.length <= room) {
      kept.push(hunk);
      used += hunk.lines.length;
      continue;
    }
    const lines = hunk.lines.slice(0, room);
    kept.push({ ...hunk, lines, splitRows: rebuildSplitRows(lines), clipped: true });
    hiddenLines += hunk.lines.length - room;
    hiddenHunks += 1;
    used += room;
  }
  return { hunks: kept, hiddenLines, hiddenHunks };
}

/** Split rows for an already-materialized line list (used when a hunk is cut
 *  mid-way; pairing is recomputed over the kept lines only). */
function rebuildSplitRows(lines: DiffLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index]!;
    if (line.kind === "removed") {
      const removed: DiffLine[] = [];
      while (index < lines.length && lines[index]!.kind === "removed") removed.push(lines[index++]!);
      const added: DiffLine[] = [];
      while (index < lines.length && lines[index]!.kind === "added") added.push(lines[index++]!);
      const count = Math.max(removed.length, added.length);
      for (let k = 0; k < count; k++) rows.push({ left: removed[k] ?? null, right: added[k] ?? null });
      continue;
    }
    rows.push({ left: line, right: line });
    index++;
  }
  return rows;
}

export function buildDiffViewModel(
  document: DiffDocument,
  options: DiffViewModelOptions = {},
): DiffViewModel {
  const wordDiff = options.wordDiff ?? true;
  const maxLines = options.maxLines ?? null;

  const views = document.hunks.map((hunk) => buildHunkView(hunk, wordDiff));
  for (let index = 0; index < views.length; index++) {
    const prev = index === 0 ? null : document.hunks[index - 1]!;
    const count = gapCount(prev, document.hunks[index]!);
    views[index]!.gapBefore = count;
    views[index]!.gapBeforeUnknown =
      count === null &&
      (index === 0
        ? false
        : prev != null && (prev.oldLines <= 0 || document.hunks[index]!.oldLines <= 0));
  }

  const totalLines = views.reduce((sum, hunk) => sum + hunk.lines.length, 0);
  const sliced = sliceHunks(views, maxLines);
  const shownLines = sliced.hunks.reduce((sum, hunk) => sum + hunk.lines.length, 0);

  return {
    hunks: sliced.hunks,
    shownLines,
    totalLines,
    hiddenLines: sliced.hiddenLines,
    hiddenHunks: sliced.hiddenHunks,
    truncated: sliced.hiddenLines > 0 || sliced.hiddenHunks > 0,
    tailGap: views.length > 0 && !options.fullFile,
    hunkCount: views.length,
    additions: document.stats.additions,
    deletions: document.stats.deletions,
  };
}

// --- Formatting helpers ------------------------------------------------------

const MODE_NAMES: Record<string, string> = {
  "040000": "directory",
  "100644": "regular file",
  "100664": "regular file, group-writable",
  "100755": "executable file",
  "120000": "symbolic link",
  "160000": "gitlink (submodule)",
};

/** `"100755"` → `"100755 (executable file)"`. Unknown modes keep the octal. */
export function formatFileMode(mode: string | null | undefined): string | null {
  if (!mode) return null;
  const name = MODE_NAMES[mode];
  return name ? `${mode} (${name})` : mode;
}

export type ModeChangeKind = "changed" | "added" | "removed";

export function modeChangeKind(document: Pick<DiffDocument, "mode">): ModeChangeKind | null {
  const change = document.mode;
  if (!change) return null;
  if (change.oldMode && change.newMode && change.oldMode !== change.newMode) return "changed";
  if (!change.oldMode && change.newMode) return "added";
  if (change.oldMode && !change.newMode) return "removed";
  return null;
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null) return "unknown size";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`;
}

/** The next context-lines step offered by the gap affordance. */
export function nextContextLines(current: number): number {
  for (const preset of CONTEXT_PRESETS) {
    if (preset > current) return preset;
  }
  return current;
}
