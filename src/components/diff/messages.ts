/**
 * Copy for the diff module (docs/04 §10: strings live in a catalogue keyed by
 * component id, never inline in templates — and the text is ours, not the
 * evidence's). Interpolating entries are functions.
 */

import type { DiffRowKind } from "./types";

const ROLE_WORD: Record<DiffRowKind, string> = {
  context: "unchanged",
  added: "added",
  removed: "removed",
  noNewline: "end of file",
};

export const diffMessages = {
  regionLabel: "File diff",
  loading: "Loading diff…",
  retry: "Retry",
  failedTitle: "The diff could not be loaded",
  emptyTitle: "No changes",
  emptyBody: "This version is identical to the one it is compared against.",
  noHunks: "The file has no textual changes to show.",

  // Toolbar
  toolbarLabel: "Diff options",
  viewLabel: "Layout",
  viewUnified: "Unified",
  viewSplit: "Split",
  contextLabel: "Context:",
  contextFull: "Whole file",
  contextLines: (n: number) => `${n} line${n === 1 ? "" : "s"}`,
  ignoreWhitespace: "Ignore whitespace",
  showFullFile: "Show whole file",
  wordDiff: "Word-level changes",
  wrap: "Wrap long lines",
  syntax: "Syntax colouring",

  // Hunk chrome
  hunkLabel: (header: string) => `Hunk ${header}`,
  collapseHunk: "Collapse hunk",
  expandHunk: "Expand hunk",
  hunkStats: (added: number, removed: number) => `+${added} −${removed}`,
  stageHunk: "Stage hunk",
  unstageHunk: "Unstage hunk",
  discardHunk: "Discard hunk",
  hunkBusy: "Working…",
  controlsDisabledReason: "Resolve the conflict before staging this file.",

  // Gaps and elision (never silently truncate)
  expandAbove: (n: number) => `Expand ${n} hidden line${n === 1 ? "" : "s"}`,
  expandAboveUnknown: "Show more context",
  expandBelow: "Show more context below",
  hiddenBannerTitle: "Part of this diff is not shown",
  hiddenBannerBody: (shown: number, total: number) =>
    `Showing ${shown} of ${total} lines.`,
  showAll: "Show all",
  backendElided: (hiddenHunks: number, hiddenLines: number) =>
    `The diff engine stopped early: ${hiddenHunks} hunk${hiddenHunks === 1 ? "" : "s"} ` +
    `(${hiddenLines} line${hiddenLines === 1 ? "" : "s"}) were not generated.`,
  requestFull: "Request full diff",
  hunkClipped: "This hunk is cut off by the render limit.",
  renderLimitNote: (limit: number) =>
    `Long diffs render the first ${limit} lines; nothing is hidden without this notice.`,

  // Binary and modes
  binaryTitle: "Binary file",
  binaryNone: "This file has no textual diff.",
  binaryIdentical: "Both sides are byte-identical; only metadata differs.",
  binaryChanged: "The binary contents differ.",
  binaryOldSize: (bytes: string) => `Before: ${bytes}`,
  binaryNewSize: (bytes: string) => `After: ${bytes}`,
  binaryOid: (side: string, oid: string) => `${side} blob ${oid}`,
  modeChanged: (from: string, to: string) => `File mode changed from ${from} to ${to}.`,
  modeAdded: (to: string) => `File created with mode ${to}.`,
  modeRemoved: (from: string) => `File deleted (was mode ${from}).`,

  // Image
  imageLoading: "Loading image data…",
  imageFailed: "Loading image data failed",
  imageUnavailable: "No image for this side",
  imageOld: "Before",
  imageNew: "After",
  imageChecker: "Transparent areas are shown over a checkerboard.",

  // Renames
  renamedFrom: (oldPath: string) => `Renamed from ${oldPath}`,

  // Accessibility (docs/04 §9: per-row labels)
  lineLabel: (kind: DiffRowKind, lineNumber: number | null, text: string) => {
    const where = lineNumber === null ? "line —" : `line ${lineNumber}`;
    const body = text.length > 200 ? `${text.slice(0, 200)}…` : text;
    return `${where}, ${ROLE_WORD[kind]}: ${body}`;
  },
  gapRowLabel: (label: string) => label,
} as const;
