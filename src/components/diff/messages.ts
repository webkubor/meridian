/**
 * Copy for the diff module (docs/04 §10: strings live in a catalogue keyed by
 * component id, never inline in templates — and the text is ours, not the
 * evidence's).
 *
 * Every entry is now a function of the caller's `t` rather than an English
 * literal: this module stays free of Vue (it is imported by plain helpers and
 * by tests that have no i18n instance), so the catalogue is injected from
 * outside instead of being reached through a global. Every key is `diff.*`.
 */

import type { DiffRowKind } from "./types";

/**
 * What `useI18n().t` looks like to a plain module. Deliberately the loose
 * signature rather than vue-i18n's `ComposerTranslation`: this file must not
 * import the runtime just to name a type, and the composer satisfies it.
 */
export type TranslateFn = (key: string, named?: Record<string, unknown>) => string;

const ROLE_KEY: Record<DiffRowKind, string> = {
  context: "diff.roleContext",
  added: "diff.roleAdded",
  removed: "diff.roleRemoved",
  noNewline: "diff.roleNoNewline",
};

export const diffMessages = {
  regionLabel: (t: TranslateFn): string => t("diff.regionLabel"),
  loading: (t: TranslateFn): string => t("diff.loading"),
  retry: (t: TranslateFn): string => t("diff.retry"),
  failedTitle: (t: TranslateFn): string => t("diff.failedTitle"),
  emptyTitle: (t: TranslateFn): string => t("diff.emptyTitle"),
  emptyBody: (t: TranslateFn): string => t("diff.emptyBody"),
  noHunks: (t: TranslateFn): string => t("diff.noHunks"),

  // Toolbar
  toolbarLabel: (t: TranslateFn): string => t("diff.toolbarLabel"),
  layoutLabel: (t: TranslateFn): string => t("diff.layoutLabel"),
  viewLabel: (t: TranslateFn): string => t("diff.viewLabel"),
  viewUnified: (t: TranslateFn): string => t("diff.viewUnified"),
  viewSplit: (t: TranslateFn): string => t("diff.viewSplit"),
  contextLabel: (t: TranslateFn): string => t("diff.contextLabel"),
  contextFull: (t: TranslateFn): string => t("diff.contextFull"),
  contextLines: (t: TranslateFn, n: number): string => t("diff.contextLines", { count: n }),
  ignoreWhitespace: (t: TranslateFn): string => t("diff.ignoreWhitespace"),
  showFullFile: (t: TranslateFn): string => t("diff.showFullFile"),
  wordDiff: (t: TranslateFn): string => t("diff.wordDiff"),
  wrap: (t: TranslateFn): string => t("diff.wrap"),
  syntax: (t: TranslateFn): string => t("diff.syntaxColouring"),

  // Hunk chrome
  hunkLabel: (t: TranslateFn, header: string): string => t("diff.hunkLabel", { header }),
  collapseHunk: (t: TranslateFn): string => t("diff.collapseHunk"),
  expandHunk: (t: TranslateFn): string => t("diff.expandHunk"),
  hunkStats: (t: TranslateFn, added: number, removed: number): string =>
    t("diff.hunkStats", { added, removed }),
  stageHunk: (t: TranslateFn): string => t("diff.stageHunk"),
  unstageHunk: (t: TranslateFn): string => t("diff.unstageHunk"),
  discardHunk: (t: TranslateFn): string => t("diff.discardHunk"),
  hunkBusy: (t: TranslateFn): string => t("diff.hunkBusy"),
  controlsDisabledReason: (t: TranslateFn): string => t("diff.controlsDisabledReason"),

  // Gaps and elision (never silently truncate)
  expandAbove: (t: TranslateFn, n: number): string => t("diff.expandAbove", { count: n }),
  expandAboveUnknown: (t: TranslateFn): string => t("diff.expandAboveUnknown"),
  expandBelow: (t: TranslateFn): string => t("diff.expandBelow"),
  hiddenBannerTitle: (t: TranslateFn): string => t("diff.hiddenBannerTitle"),
  hiddenBannerBody: (t: TranslateFn, shown: number, total: number): string =>
    t("diff.hiddenBannerBody", { shown, total }),
  showAll: (t: TranslateFn): string => t("diff.showAll"),
  backendElided: (t: TranslateFn, hiddenHunks: number, hiddenLines: number): string =>
    t("diff.backendElided", { count: hiddenHunks, lines: hiddenLines }),
  requestFull: (t: TranslateFn): string => t("diff.requestFull"),
  hunkClipped: (t: TranslateFn): string => t("diff.hunkClipped"),
  renderLimitNote: (t: TranslateFn, limit: number): string =>
    t("diff.renderLimitNote", { count: limit }),

  // Binary and modes
  binaryTitle: (t: TranslateFn): string => t("diff.binaryTitle"),
  binaryNone: (t: TranslateFn): string => t("diff.binaryNone"),
  binaryIdentical: (t: TranslateFn): string => t("diff.binaryIdentical"),
  binaryChanged: (t: TranslateFn): string => t("diff.binaryChanged"),
  binaryOldSize: (t: TranslateFn, bytes: string): string => t("diff.binaryOldSize", { size: bytes }),
  binaryNewSize: (t: TranslateFn, bytes: string): string => t("diff.binaryNewSize", { size: bytes }),
  binaryOid: (t: TranslateFn, side: string, oid: string): string => t("diff.binaryOid", { side, oid }),
  modeChanged: (t: TranslateFn, from: string, to: string): string => t("diff.modeChanged", { from, to }),
  modeAdded: (t: TranslateFn, to: string): string => t("diff.modeAdded", { to }),
  modeRemoved: (t: TranslateFn, from: string): string => t("diff.modeRemoved", { from }),

  // Image
  imageLoading: (t: TranslateFn): string => t("diff.imageLoading"),
  imageFailed: (t: TranslateFn): string => t("diff.imageFailed"),
  imageUnavailable: (t: TranslateFn): string => t("diff.imageUnavailable"),
  imageOld: (t: TranslateFn): string => t("diff.before"),
  imageNew: (t: TranslateFn): string => t("diff.after"),
  imageChecker: (t: TranslateFn): string => t("diff.imageChecker"),

  // Renames
  renamedFrom: (t: TranslateFn, oldPath: string): string => t("diff.renamedFrom", { path: oldPath }),

  // Accessibility (docs/04 §9: per-row labels)
  lineLabel: (t: TranslateFn, kind: DiffRowKind, lineNumber: number | null, text: string): string => {
    const where =
      lineNumber === null ? t("diff.lineNumberUnknown") : t("diff.lineNumber", { number: lineNumber });
    const body = text.length > 200 ? `${text.slice(0, 200)}…` : text;
    return t("diff.lineLabel", { where, role: t(ROLE_KEY[kind]), text: body });
  },
} as const;