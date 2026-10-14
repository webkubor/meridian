/**
 * View-model contracts for the UI layer.
 *
 * Everything a screen renders arrives through props typed here. The shapes that
 * already exist in `bridges/bindings.ts` (RepoInfo, CommitInfo, BranchInfo,
 * EntryStatus, …) are imported, never re-declared; the shapes below are the
 * ones this layer needs from modules the UI agent does not own
 * (`stores/diff`, `stores/graph`, `stores/compare`, `stores/blame`,
 * `stores/settings`, `stores/services`, `stores/activities`, …).
 *
 * Each block names the store and the docs/03 rows it mirrors, so the owning
 * module can satisfy it without reading this layer's components.
 */

import type {
  BranchInfo,
  CommitInfo,
  FileStatus,
  IntegrationState,
  RepoInfo,
  Tone,
} from "../bridges/bindings";
import type { LoadState } from "../stores/loadState";

export type { LoadState };

/* ========================================================================== */
/* Graph — expected from `stores/graph.ts`                                    */
/*   history_graph_compute (docs/03 §5) → GraphLayout                          */
/* ========================================================================== */

export type GraphEdgeKind = "straight" | "merge" | "fork" | "bend";

/**
 * One drawn lane segment. `lane`/`toLane` are 0-based; when the kind is
 * `straight` they are equal. A segment spans row indices of the loaded page.
 *
 * NOTE for the graph store (contract expectation): docs/02 §13 describes the
 * engine tuple as `(lane, from_row, to_row, edge_kind)`, which cannot address
 * the *far lane* of a diagonal segment. The UI needs it, so the shape here adds
 * `toLane`; the store should emit `toLane: lane` for straight segments.
 */
export interface GraphLaneSegment {
  lane: number;
  toLane: number;
  fromRow: number;
  toRow: number;
  kind: GraphEdgeKind;
}

export interface GraphLayout {
  /** Number of lanes in use (≥ 1); the column is `laneCount * laneWidth`. */
  laneCount: number;
  segments: GraphLaneSegment[];
  /**
   * Bubble lane per layout row (parallel to the commit array). Present when the
   * engine reports it; the commit list falls back to the `graphRows` prop when
   * it is absent.
   */
  lanes?: number[];
}

export type GraphRowKind = "commit" | "merge" | "forkPoint" | "dateGroup";

/** Row geometry the canvas needs; `lane: null` for rows with no bubble. */
export interface GraphRow {
  kind: GraphRowKind;
  lane: number | null;
}

/* ========================================================================== */
/* Diff — expected from `stores/diff.ts`                                      */
/*   wc_diff_load / history_diff_document_load (docs/03 §3, §5) → DiffDocument */
/* ========================================================================== */

export type DiffPerspective =
  | "workdir-vs-index"
  | "index-vs-head"
  | "workdir-vs-head"
  | "commit-vs-parent"
  | "range"
  | "merge-base";

export type DiffViewMode = "unified" | "split";

export interface DiffOptions {
  contextLines: number;
  ignoreWhitespace: boolean;
  fullFile: boolean;
  syntax: boolean;
}

export interface HunkRef {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
}

/** Word-level emphasis inside a changed line (paired removal/addition). */
export interface WordSpan {
  text: string;
  emphasized: boolean;
}

export interface DiffLineNumbers {
  oldLine: number | null;
  newLine: number | null;
}

export type DiffRow =
  | ({ kind: "hunkHeader"; header: string; hunk: HunkRef } & DiffLineNumbers)
  | ({ kind: "context"; text: string; spans?: WordSpan[] } & DiffLineNumbers)
  | ({ kind: "added"; text: string; spans?: WordSpan[] } & DiffLineNumbers)
  | ({ kind: "removed"; text: string; spans?: WordSpan[] } & DiffLineNumbers)
  | { kind: "noNewline"; side: "old" | "new" };

export interface DiffStats {
  additions: number;
  deletions: number;
}

export interface DiffDocument {
  path: string;
  oldPath: string | null;
  perspective: DiffPerspective;
  rows: DiffRow[];
  stats: DiffStats;
  /** Set for binary content; the viewer renders the "binary" state. */
  binary?: boolean;
  /** Set for image content the viewer cannot render as text. */
  image?: { oldSize: number | null; newSize: number | null };
  /** Set when the diff exceeds the gate; the viewer renders "Show anyway". */
  tooLarge?: { bytes: number; gate: number };
}

export type HunkAction = "stage" | "unstage" | "discard";

/* ========================================================================== */
/* Commit detail / changeset — expected from `stores/history.ts` additions     */
/*   history_commit_load, history_changeset_load (docs/03 §5)                  */
/* ========================================================================== */

export type SignatureState = "unsigned" | "present" | "valid" | "untrusted" | "invalid" | "expired";

export interface CommitSignature {
  state: SignatureState;
  signer?: string;
  keyId?: string;
}

export interface RefChip {
  name: string;
  kind: "head" | "branch" | "remoteBranch" | "tag" | "other";
}

export interface CommitDetail {
  commit: CommitInfo;
  refs: RefChip[];
  signature: CommitSignature;
}

export interface ChangesetEntry {
  path: string;
  oldPath: string | null;
  status: FileStatus;
  additions: number;
  deletions: number;
  binary: boolean;
}

export interface Changeset {
  entries: ChangesetEntry[];
  stats: { files: number; additions: number; deletions: number };
}

/* ========================================================================== */
/* Compare — expected from `stores/compare.ts`                                */
/*   history_compare_revisions (docs/03 §5) → Comparison                       */
/* ========================================================================== */

export type CompareSide = "ahead" | "behind";

export interface CompareEntry {
  side: CompareSide;
  commit: CommitInfo;
}

export interface Comparison {
  left: string;
  right: string;
  mergeBase: string | null;
  ahead: number;
  behind: number;
  entries: CompareEntry[];
}

/** A selectable ref in the compare screen's two pickers. */
export interface RefChoice {
  id: string;
  label: string;
  detail?: string;
  kind: "head" | "branch" | "remoteBranch" | "tag";
}

/* ========================================================================== */
/* Blame — expected from `stores/blame.ts`                                    */
/*   history_blame_load (docs/03 §5) → BlameLines                             */
/* ========================================================================== */

export interface BlameLine {
  line: number;
  oid: string;
  shortOid: string;
  author: string;
  authorMail?: string;
  time: number;
  text: string;
  /** True on the first line of a hunk; drives gutter grouping. */
  hunkStart: boolean;
  hunkSize: number;
  /** The commit that moved this line in from another file, when known. */
  previous: { oid: string; path: string } | null;
}

export interface BlameLines {
  path: string;
  lines: BlameLine[];
  /** Row id per line, so the same hunk can be tinted consistently. */
  hunkIds: number[];
  summaries: Record<string, { summary: string; author: string; time: number }>;
}

/** The popover opened by clicking a blame annotation. */
export interface BlamePopover {
  line: number;
  oid: string;
  shortOid: string;
  summary: string;
  author: string;
  time: number;
}

/* ========================================================================== */
/* Working copy extras — expected from `stores/workingCopy.ts` additions      */
/*   wc_merge_status_load, wc_conflicts_load (docs/03 §3)                     */
/* ========================================================================== */

export interface MergeStatus {
  state: "mergeable" | "conflict" | "merged";
  incoming: { label: string; commits: number } | null;
  base: string | null;
  conflicts: { total: number; resolved: number };
}

/* ========================================================================== */
/* Settings — expected from `stores/settings.ts`                              */
/*   settings_get_all → SettingsDocument; settings_set; appearance_set (docs/03 §17) */
/* ========================================================================== */

export type SettingValue = boolean | number | string;

export type SettingKind = "bool" | "int" | "string" | "text" | "enum" | "path" | "color";

export interface SettingOption {
  value: string;
  label: string;
}

export interface SettingRowModel {
  key: string;
  label: string;
  kind: SettingKind;
  value: SettingValue;
  defaultValue?: SettingValue;
  help?: string;
  placeholder?: string;
  options?: SettingOption[];
  min?: number;
  max?: number;
  /** True while the row's value is shell-owned (no schema behind it yet). */
  shellOwned?: boolean;
}

export interface SettingsGroupModel {
  id: string;
  label: string;
  rows: SettingRowModel[];
}

export interface SettingsTabModel {
  id: string;
  label: string;
  icon: string;
  groups: SettingsGroupModel[];
  /** `schema` when built from settings_get_all, `shell` for shell-local rows. */
  source: "schema" | "shell";
}

export interface SettingsDocument {
  tabs: SettingsTabModel[];
  version: number;
}

export type AppearanceMode = "auto" | "light" | "dark";

/* ========================================================================== */
/* Services / accounts — expected from `stores/services.ts`                   */
/*   account_list / service_* (docs/03 §15)                                   */
/* ========================================================================== */

export type ProviderId =
  | "github"
  | "gitlab"
  | "bitbucket"
  | "azureDevOps"
  | "stash"
  | "beanstalk"
  | "custom";

export interface AccountKeySummary {
  id: string;
  title: string;
  fingerprint?: string;
}

export interface ServiceAccount {
  id: string;
  provider: ProviderId;
  login: string;
  displayName: string;
  authMethod: string;
  connected: boolean;
  organizations: string[];
  keys: AccountKeySummary[];
  repositoryCount: number;
}

/* ========================================================================== */
/* Activities — expected from `stores/activities.ts`                          */
/*   remote_activity_* (docs/03 §20) → ActivityItem                            */
/* ========================================================================== */

export type ActivityKind = "remote" | "clone" | "service" | "indexer" | "operation";
export type ActivityState = "running" | "stalled" | "finished" | "cancelled" | "error";

export interface ActivityItem {
  id: string;
  kind: ActivityKind;
  title: string;
  detail?: string;
  repoLabel?: string;
  state: ActivityState;
  progress: { done: number; total: number } | null;
  phase: string | null;
  startedAt: number;
  elapsedMs: number | null;
  log: string[];
}

/* ========================================================================== */
/* Shell models — owned by the UI layer                                       */
/* ========================================================================== */

export interface ToolbarVerbModel {
  id: string;
  label: string;
  icon: string;
  group: "global" | "utility";
  disabled: boolean;
  reason?: string;
  busy?: boolean;
  options?: SplitActionOption[];
}

/** One row in a `<SplitAction>` caret panel (default action + options). */
export interface SplitActionOption {
  id: string;
  label: string;
  danger?: boolean;
  disabled?: boolean;
  reason?: string;
}

/** One toggleable row inside a `<FilterMenu>` (per-pane view settings). */
export interface FilterOptionModel {
  id: string;
  label: string;
  checked: boolean;
  hint?: string;
}

export interface FilterSectionModel {
  id: string;
  label: string;
  /** `radio` sections are exclusive; the parent enforces that on `toggle`. */
  kind: "radio" | "check";
  options: FilterOptionModel[];
}

export interface SidebarBadgeModel {
  id: string;
  icon?: string;
  label?: string;
  tone: Tone;
}

export interface SidebarRowModel {
  id: string;
  label: string;
  icon?: string;
  detail?: string;
  badges: SidebarBadgeModel[];
  group?: string;
  current?: boolean;
  hasOptions?: boolean;
}

export interface SidebarSectionModel {
  id: string;
  title: string;
  icon: string;
  rows: SidebarRowModel[];
  collapsed: boolean;
  /** Rendered when the section has no rows (the filtered-empty state). */
  emptyCaption: string;
  /** Pseudo-sections hold a single always-present row (Working copy). */
  pseudo?: boolean;
}

export interface ToastModel {
  id: string;
  kind: "info" | "success" | "warning" | "danger";
  text: string;
  actionLabel?: string;
}

export interface PaletteItemModel {
  id: string;
  label: string;
  detail?: string;
  icon?: string;
  group: string;
  disabled?: boolean;
  reason?: string;
}

/* ---- Dialogues ----------------------------------------------------------- */

export type DialogueRequest =
  | {
      kind: "confirm";
      id: string;
      title: string;
      message: string;
      /** Itemised list of what is affected (docs/04 §8.2). */
      items?: string[];
      confirmLabel: string;
      danger?: boolean;
    }
  | {
      kind: "prompt";
      id: string;
      title: string;
      subtitle?: string;
      label: string;
      value: string;
      placeholder?: string;
      helpText?: string;
      /** Named validator so requests stay serialisable. */
      validate?: "required" | "refname";
      option?: { label: string; checked: boolean; helpText?: string };
    }
  | {
      kind: "chooseBranch";
      id: string;
      title: string;
      subtitle?: string;
      branches: BranchInfo[];
      current: string | null;
      /** Return `fullName` so remote refs stay distinct from same-named locals. */
      emitFullName?: boolean;
    };

export interface DialogueResult {
  id: string;
  kind: DialogueRequest["kind"];
  /** prompt: the text; chooseBranch: the branch name; confirm: empty. */
  value: string;
  /** prompt with an option row: the checkbox state. */
  optionChecked: boolean;
}

/* ---- View helpers -------------------------------------------------------- */

/** The extra review data the branches screen renders (branch_review_load). */
export interface BranchReviewInfo extends BranchInfo {
  fullyMerged?: boolean;
  lastAuthor?: string;
}

export interface HistorySearchModel {
  text: string;
  author: string;
  since: string;
  until: string;
  type: "all" | "merges" | "nonMerges";
}

export interface HistoryViewOptions {
  showGraph: boolean;
  density: "compact" | "regular";
  sort: "date" | "topo";
  firstParent: boolean;
  /** Date-group headers between commits (docs/04 §S8). */
  dateGrouping: "off" | "day" | "week" | "month";
}

/** Repository screen context passed down to panes. */
export interface RepoSummaryModel {
  info: RepoInfo;
  headLabel: string | null;
  state: IntegrationState;
}