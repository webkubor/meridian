/**
 * Typed mirror of the Rust command surface (docs/02 §2 `bridges/`).
 *
 * Hand-written for P0 and kept in sync by review; docs/02 §1 schedules
 * `tauri-specta` to generate this file from the Rust commands once the surface
 * stabilises. Field names are camelCase on the wire — the Rust structs carry
 * `#[serde(rename_all = "camelCase")]` — and codes stay snake_case because
 * they are stable machine identifiers.
 */

export type RepoId = string;

export type ErrorDomain = "git" | "service" | "fs" | "internal" | "cancelled";

export type ErrorCode =
  | "invalid_repository"
  | "no_working_copy"
  | "io"
  | "invalid_request"
  | "index_locked"
  | "checkout_conflicts"
  | "checkout_failed"
  | "unknown_revision"
  | "invalid_reference_name"
  | "reference_exists"
  | "reference_not_found"
  | "operation_in_progress"
  | "cancelled"
  | "unclassified";

export interface ErrorPayload {
  code: ErrorCode;
  message: string;
  detail?: string;
  params?: Record<string, string>;
  retryable: boolean;
  domain: ErrorDomain;
}

export type SenderTag =
  | "menu"
  | "toolbar"
  | "sidebarContextMenu"
  | "quickActionPalette"
  | "dialogue"
  | "cli"
  | "api";

export type Revision =
  | { kind: "head" }
  | { kind: "branch"; value: string }
  | { kind: "tag"; value: string }
  | { kind: "oid"; value: string };

export type IntegrationState =
  | "clean"
  | "merge"
  | "revert"
  | "revertSequence"
  | "cherryPick"
  | "cherryPickSequence"
  | "bisect"
  | "rebase"
  | "rebaseInteractive"
  | "rebaseMerge"
  | "applyMailbox"
  | "applyMailboxOrRebase"
  | "other";

export interface RepoInfo {
  id: RepoId;
  gitdir: string;
  workdir: string | null;
  isBare: boolean;
  isEmpty: boolean;
  headBranch: string | null;
  headOid: string | null;
  state: IntegrationState;
  discovered: boolean;
}

export type FileStatus =
  | "added"
  | "copied"
  | "deleted"
  | "modified"
  | "renamed"
  | "unmerged"
  | "untracked"
  | "ignored";

export type Tone = "neutral" | "positive" | "info" | "danger" | "warning" | "muted";

export interface UiStatusView {
  /** One-character badge: A / C / D / M / R / U / ? / ! */
  short: string;
  text: string;
  tone: Tone;
}

export interface EntryStatus {
  path: string;
  code: FileStatus;
  staged: boolean;
  assumeUnchanged: boolean;
  view: UiStatusView;
}

export interface StatusSummary {
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
}

export interface StatusDelta {
  repo: RepoId;
  entries: EntryStatus[];
  summary: StatusSummary;
}

export interface Signature {
  name: string;
  email: string;
  time: number;
}

export interface CommitInfo {
  oid: string;
  shortOid: string;
  parents: string[];
  summary: string;
  message: string;
  author: Signature;
  committer: Signature;
  decorations: string[];
}

export interface CommitPage {
  commits: CommitInfo[];
  truncated: boolean;
}

/** Exact ahead/behind counts and merge base between two revisions. */
export interface CompareSummary {
  mergeBase: string | null;
  ahead: number;
  behind: number;
}

export type HistorySort = "date" | "topo";

/** Which part of a row `history_commits_load`'s `search` matches (Rust `SearchMode`). */
export type HistorySearchMode = "all" | "message" | "author" | "hash";

export type BranchFilter = "local" | "remote" | "all";

export interface BranchInfo {
  name: string;
  fullName: string;
  oid: string;
  isHead: boolean;
  isRemote: boolean;
  upstream?: string;
  ahead: number;
  behind: number;
  tipSummary: string;
  tipTime: number;
}

export interface CheckoutResult {
  branch: string | null;
  oid: string;
  detached: boolean;
}

/** Command argument shapes — one interface per command, mirroring docs/03. */

export interface RepositoryInitOptions {
  bare?: boolean;
  initialBranch?: string;
  createGitignore?: boolean;
}

/**
 * A repository found by `repository_discover` — one row of "found nearby".
 *
 * `RepoRoot` is deliberately not `RepoInfo`: nothing is open yet, so there is
 * no `id` and nothing to run a command against. Opening it is what turns a
 * root into a repository.
 */
export interface RepoRoot {
  name: string;
  path: string;
  gitdir: string;
  workdir: string | null;
  isBare: boolean;
  isWorktree: boolean;
}

export interface RepositoryDiscoverOptions {
  /** Depth below the root to inspect; `0` means no limit. */
  maxDepth?: number;
  /** Directory names never entered, exact matches. */
  ignored?: string[];
}

export interface RepositoryCloneOptions {
  destination?: string;
  bare?: boolean;
  initialBranch?: string;
}

export interface WcStatusOptions {
  includeIgnored?: boolean;
  includeDirs?: boolean;
}

export interface HistoryCommitsOptions {
  from?: Revision;
  to?: Revision;
  path?: string;
  skip?: number;
  limit?: number;
  sort?: HistorySort;
  firstParent?: boolean;
  followRenames?: boolean;
  /** Case-insensitive substring matched per `searchMode`. */
  search?: string;
  searchMode?: HistorySearchMode;
}

export interface CheckoutOptions {
  detach?: boolean;
}

/** The app-wide event bus frames emitted by the Rust side (`events.rs`). */

export type AppEvent =
  | { kind: "reposChanged" }
  | { kind: "refsChanged"; payload: { repo: RepoId } }
  | { kind: "workingCopyChanged"; payload: { repo: RepoId } };

/* ========================================================================== */
/* Commit detail / changeset — `history_commit_load`, `history_changeset_load` */
/* (Rust: domain/commit.rs — plain serde structs, camelCase on the wire)       */
/* ========================================================================== */

/** A ref chip on a row, typed (`CommitRow::refs` / `CommitDetail::refs`). */
export type RefDecorationKind = "head" | "branch" | "remote" | "tag" | "other";

export interface RefDecoration {
  /** Short display name: `main`, `origin/main`, `v1.2`. */
  name: string;
  /** Full refname (`refs/heads/main`). */
  fullName: string;
  kind: RefDecorationKind;
  /** True for the branch HEAD points at. */
  isHead: boolean;
}

/** Whether the commit object carries a signature, and in which format. */
export type CommitSignatureState = "unsigned" | "pgp" | "ssh" | "other";

export interface Coauthor {
  name: string;
  email: string;
}

export interface Trailer {
  token: string;
  value: string;
}

export interface ChangesetStats {
  filesChanged: number;
  insertions: number;
  deletions: number;
}

/** `history_commit_load` output (Rust `domain::commit::CommitDetail`). */
export interface CommitDetail {
  oid: string;
  shortOid: string;
  parents: string[];
  parentCount: number;
  tree: string;
  summary: string;
  body: string;
  message: string;
  author: Signature;
  committer: Signature;
  decorations: string[];
  refs: RefDecoration[];
  signature: CommitSignatureState;
  coauthors: Coauthor[];
  trailers: Trailer[];
  stats: ChangesetStats;
}

export interface ChangesetEntry {
  path: string;
  oldPath?: string;
  status: FileStatus;
  binary: boolean;
  insertions: number;
  deletions: number;
}

/** `history_changeset_load` output (Rust `domain::commit::Changeset`). */
export interface Changeset {
  oid: string;
  parent: string | null;
  from?: string;
  entries: ChangesetEntry[];
  stats: ChangesetStats;
}

/* ========================================================================== */
/* Graph — `history_graph_compute` (Rust: domain/commit.rs `GraphLayout`)      */
/* ========================================================================== */

export type GraphEdgeKind = "straight" | "bend" | "merge" | "fork";

export interface GraphLaneSegment {
  fromRow: number;
  toRow: number;
  lane: number;
  fromLane: number;
  toLane: number;
  kind: GraphEdgeKind;
  starting: boolean;
  ending: boolean;
}

export interface GraphLayout {
  /** Bubble lane per layout row; `-1` marks a row with no commit. */
  lanes: number[];
  segments: GraphLaneSegment[];
  laneCount: number;
}

/* ========================================================================== */
/* Diff — `wc_diff_load` / `wc_diff_hunks_load`                               */
/* (Rust: domain/working_copy.rs — Diff{Document,Hunk,Line}, FileChange)       */
/* ========================================================================== */

export type DiffLineKind = "context" | "added" | "removed" | "noNewline";

export interface DiffLine {
  kind: DiffLineKind;
  oldLineno?: number;
  newLineno?: number;
  text: string;
}

export interface DiffHunk {
  header: string;
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: DiffLine[];
}

/** The six diff perspectives (Rust `DiffPerspective`). */
export type DiffPerspective =
  | "indexToBase"
  | "mineToMergeBase"
  | "mineToTheirs"
  | "theirsToMergeBase"
  | "workingTreeToBase"
  | "workingTreeToIndex";

export type FileMode = "regular" | "executable" | "symlink" | "submodule" | "tree" | "unreadable";

export interface FileModeChange {
  old: FileMode;
  new: FileMode;
}

export interface FileChange {
  status: FileStatus;
  oldPath?: string;
  modeChange?: FileModeChange;
  oldSize: number;
  newSize: number;
}

/** `wc_diff_load` output (Rust `domain::working_copy::DiffDocument`). */
export interface DiffDocument {
  path?: string | null;
  perspective: DiffPerspective;
  change?: FileChange;
  isBinary: boolean;
  hunks: DiffHunk[];
  additions: number;
  deletions: number;
  truncated: boolean;
  syntax?: string;
}

/** `wc_diff_load` options (Rust `DiffOptions`). */
export interface DiffOptionsWire {
  contextLines?: number;
  ignoreWhitespace?: boolean;
  showFull?: boolean;
  syntax?: string;
}

/** `wc_chunk_apply` / `wc_chunk_discard` hunk identity (Rust `HunkRef`). */
export interface HunkRef {
  path: string;
  perspective: DiffPerspective;
  index: number;
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  header: string;
}

export type ChunkDirection = "stage" | "unstage";

/** `wc_image_data_load` output (Rust `domain::working_copy::ImageData`). */
export type ImageSide = "old" | "new";

export interface ImageData {
  path: string;
  side: ImageSide;
  mime: string;
  size: number;
  /** Base64 of the raw file bytes; empty when that side does not exist. */
  dataBase64: string;
}

/** `wc_entry_open` target (Rust `OpenTarget`). */
export type OpenTarget = "editor" | "terminal" | "finder" | "diffTool";

/** `wc_paths_discard` options (Rust `DiscardOptions`). */
export interface DiscardOptions {
  includeUntracked?: boolean;
  confirmToken?: string;
}

/* ========================================================================== */
/* Commits — `commit_create` / `commit_amend` (docs/03 §4)                     */
/* ========================================================================== */

export type CommitSign = { kind: "gpg"; key?: string } | { kind: "ssh"; key?: string } | { kind: "none" };

export interface CommitOptions {
  signOff?: boolean;
  sign?: CommitSign;
  /**
   * Accepted and carried through, but the hook chain is not run in this build
   * (see `docs/03 §4`); the engine ignores it until hooks land.
   */
  skipHooks?: boolean;
  /** A template supplied the message (drives the trailer policy). */
  templateUsed?: boolean;
  committer?: Signature;
}

/** `commit_create` / `commit_amend` output (Rust `domain::commit::CommitResult`). */
export interface CommitResult {
  oid: string;
  shortOid: string;
  parents: string[];
  parentCount: number;
  summary: string;
  message: string;
  author: Signature;
  committer: Signature;
  /** The branch the commit landed on; absent for a detached HEAD. */
  branch?: string;
  detached: boolean;
  amend: boolean;
}

/* ========================================================================== */
/* Remotes — `remote_*` (docs/03 §8)                                          */
/* ========================================================================== */

export type RemoteFetchMode = "default" | "prune" | "tags" | "pruneTags";

export interface PullOptions {
  mode?: "merge" | "rebase" | "ffOnly";
  message?: string;
}

export interface RepositoryRemote {
  name: string;
  fetchUrl: string | null;
  pushUrl: string | null;
  fetchRefspecs: string[];
  pushRefspecs: string[];
  fetchMode: RemoteFetchMode;
  trackedBranches: string[];
}

export interface RepositoryRemotes {
  remotes: RepositoryRemote[];
  upstreams: UpstreamTracking[];
}

export interface UpstreamTracking {
  branch: string;
  remote: string | null;
  remoteBranch: string | null;
  mergeRef: string | null;
  ahead: number;
  behind: number;
  gone: boolean;
}

/* ========================================================================== */
/* Integrate — merge / rebase / conflicts (docs/03 §12)                        */
/* ========================================================================== */

export type IntegrationOutcome =
  | { kind: "upToDate"; head: string }
  | { kind: "fastForward"; head: string; from: string }
  | { kind: "merged"; commit: string; parents: string[] }
  | { kind: "staged" }
  | { kind: "conflicted" }
  | { kind: "edited"; commit: string }
  | { kind: "rebaseFinished"; head: string; applied: number; dropped: number }
  | { kind: "sequenceFinished"; applied: string[]; skipped: string[]; dropped: string[] }
  | { kind: "aborted"; head: string };

export interface IntegrationResult {
  outcome: IntegrationOutcome;
  conflicts: unknown[];
  notes: string[];
}

export type MergeStrategy = "default" | "ours" | "theirs" | "union";

export interface MergeOptions {
  alwaysMergeCommit?: boolean;
  autoCommit?: boolean;
  verifySignatures?: boolean;
  autostash?: boolean;
  squash?: boolean;
  strategy?: MergeStrategy;
  message?: string;
}

export interface RebaseOptions {
  autostash?: boolean;
  verifySignatures?: boolean;
}

export type MergeState = "mergeable" | "conflict" | "merged";

export interface MergeStatus {
  state: MergeState;
  incoming?: string;
  base?: string;
  fastForward: boolean;
  conflictPreview: string[];
}

export type ResolutionStrategy =
  | "ours"
  | "theirs"
  | "base"
  | "both"
  | { manual: { content: string } }
  | "saveVersions"
  | "markResolved";

/* ========================================================================== */
/* Stash — `stash_save` (docs/03 §10)                                          */
/* ========================================================================== */

export interface StashSaveOptions {
  message?: string;
  includeUntracked?: boolean;
  includeIgnored?: boolean;
  keepIndex?: boolean;
  paths?: string[];
}

/* ========================================================================== */
/* Application — `app_*` (Rust: domain/app.rs)                                */
/*                                                                            */
/* The one command family addressed to no repository. Everything above is     */
/* per-repo; everything here is about the app itself: which version is        */
/* running, what changed in it, whether a newer one exists, and where to send */
/* a complaint.                                                               */
/* ========================================================================== */

/** Rust `BuildProfile` — snake_case on the wire. */
export type BuildProfile = "debug" | "release";

/** Rust `NotesLocale` — snake_case on the wire. */
export type NotesLocale = "zh_cn" | "en";

/** `app_info_load` output (Rust `domain::app::AppInfo`). */
export interface AppInfo {
  name: string;
  version: string;
  /** Absent when the build came from a source tarball rather than a checkout. */
  commit?: string | null;
  buildProfile: BuildProfile;
  rustVersion: string;
  osName: string;
  osVersion: string;
  arch: string;
  /** `owner/name`, absent when the build had no `origin` remote. */
  repoSlug?: string | null;
  updatesConfigured: boolean;
  /** False for debug builds and for any build with no endpoint. */
  canSelfUpdate: boolean;
  /**
   * The text behind "Copy diagnostics", composed in Rust.
   *
   * Sent rather than assembled here: two formatters would drift, and the one the
   * user pastes into a public issue is the one nobody would be testing. Read it,
   * do not rebuild it.
   */
  diagnostics: string;
}

/** `app_release_notes_load` argument shape (Rust `ReleaseNotesLoad`). */
export interface ReleaseNotesLoad {
  /** The UI's active language. Unknown values fall back to English. */
  locale?: string | null;
}

/** `app_release_notes_load` output (Rust `domain::app::ReleaseNotes`). */
export interface ReleaseNotes {
  /** The changelog verbatim, as Markdown. */
  markdown: string;
  locale: NotesLocale;
  /** The version this build is, so the UI can mark where the reader is. */
  currentVersion: string;
}

/**
 * `app_update_check` output (Rust `domain::app::UpdateStatus`).
 *
 * `available` is deliberately not a field: it is derived by `updateAvailable()`
 * below from the two versions, exactly as the Rust side derives it, so the two
 * can never disagree about whether an update exists.
 */
export interface UpdateStatus {
  currentVersion: string;
  latestVersion?: string | null;
  /** The release body from the update manifest, if it carried one. */
  notes?: string | null;
  publishedAt?: string | null;
  downloadBytes?: number | null;
}

/**
 * `app_update_check` result (Rust `domain::app::UpdateCheck`, tagged `kind`).
 *
 * A build that cannot check still answers *why* — otherwise the UI shows a
 * spinner forever or a button that always errors.
 */
export type UpdateCheck =
  | { kind: "checked"; status: UpdateStatus }
  | { kind: "notConfigured" }
  | { kind: "unsupportedInDebugBuild" };

/**
 * Frames pushed on the install channel (Rust `commands::app::UpdateProgress`).
 *
 * A channel rather than polling: a 60 MB download polled at any sane rate is
 * either laggy or hundreds of invocations.
 */
export type UpdateProgress =
  | { phase: "downloading"; received: number; total?: number | null }
  | { phase: "downloaded" }
  | { phase: "installing" }
  | { phase: "readyToRestart" };

/** Rust `FeedbackKind` — which issue form to open. */
export type FeedbackKind = "bug" | "feature";

/** `app_feedback_compose` argument shape (Rust `FeedbackRequest`). */
export interface FeedbackRequest {
  kind: FeedbackKind;
  /** A title the user typed, if any. Becomes a query parameter, nothing more. */
  summary?: string | null;
}

/**
 * `app_feedback_compose` output (Rust `FeedbackTarget`).
 *
 * Returned rather than opened in the backend so the caller can show the user
 * what is about to be attached *before* a browser opens.
 */
export interface FeedbackTarget {
  url: string;
  /** The diagnostic lines that were prefilled, for display. */
  prefilled: string[];
}

/**
 * Whether a newer release exists. Mirrors `UpdateStatus::available()` in Rust:
 * an update exists only when a latest version is known *and differs* from the
 * running one. A build must never tell the user to update to what they have.
 */
export function updateAvailable(status: UpdateStatus): boolean {
  return status.latestVersion != null && status.latestVersion !== status.currentVersion;
}

