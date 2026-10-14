/**
 * The only call site of `invoke` in the app (docs/02 §13): stores and
 * composables go through these typed wrappers, so docs/03 stays authoritative.
 *
 * Tauri maps the `args` object onto each command's single argument struct —
 * `invoke("branch_create", { args: { repo, name, start, checkout, sender } })`.
 */

import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  AppInfo,
  BranchFilter,
  BranchInfo,
  Changeset,
  CheckoutOptions,
  CheckoutResult,
  ChunkDirection,
  CommitDetail,
  CommitOptions,
  CommitPage,
  CommitResult,
  DiffDocument,
  DiffOptionsWire,
  DiffPerspective,
  DiscardOptions,
  FeedbackRequest,
  FeedbackTarget,
  GraphLayout,
  HistoryCommitsOptions,
  HunkRef,
  ImageData,
  ImageSide,
  IntegrationResult,
  MergeOptions,
  MergeStatus,
  OpenTarget,
  PullOptions,
  RebaseOptions,
  ReleaseNotes,
  RemoteFetchMode,
  RepoId,
  RepoInfo,
  RepositoryInitOptions,
  RepositoryRemotes,
  Revision,
  SenderTag,
  StashSaveOptions,
  StatusDelta,
  UpdateCheck,
  UpdateProgress,
  UpdateStatus,
  UpstreamTracking,
  WcStatusOptions,
} from "./bindings";

export function repositoryOpen(path: string, sender: SenderTag = "dialogue"): Promise<RepoInfo> {
  return invoke<RepoInfo>("repository_open", { args: { path, sender } });
}

export function repositoryInit(
  path: string,
  options: RepositoryInitOptions = {},
): Promise<RepoInfo> {
  return invoke<RepoInfo>("repository_init", { args: { path, ...options } });
}

export function wcStatusLoad(repo: RepoId, opts: WcStatusOptions = {}): Promise<StatusDelta> {
  return invoke<StatusDelta>("wc_status_load", { args: { repo, opts } });
}

export function wcStagePaths(repo: RepoId, paths: string[]): Promise<StatusDelta> {
  return invoke<StatusDelta>("wc_stage_paths", { args: { repo, paths } });
}

export function wcUnstagePaths(repo: RepoId, paths: string[]): Promise<StatusDelta> {
  return invoke<StatusDelta>("wc_unstage_paths", { args: { repo, paths } });
}

export function historyCommitsLoad(
  repo: RepoId,
  options: HistoryCommitsOptions = {},
): Promise<CommitPage> {
  return invoke<CommitPage>("history_commits_load", { args: { repo, ...options } });
}

export function branchList(
  repo: RepoId,
  filter: BranchFilter = "local",
  search?: string,
): Promise<BranchInfo[]> {
  return invoke<BranchInfo[]>("branch_list", { args: { repo, filter, search } });
}

export function branchCreate(
  repo: RepoId,
  name: string,
  start: Revision = { kind: "head" },
  checkout = true,
  sender: SenderTag = "dialogue",
): Promise<BranchInfo> {
  return invoke<BranchInfo>("branch_create", { args: { repo, name, start, checkout, sender } });
}

export function branchCheckout(
  repo: RepoId,
  name: string,
  opts: CheckoutOptions = {},
): Promise<CheckoutResult> {
  return invoke<CheckoutResult>("branch_checkout", { args: { repo, name, opts } });
}

export function branchRename(
  repo: RepoId,
  old: string,
  next: string,
  force = false,
): Promise<BranchInfo> {
  return invoke<BranchInfo>("branch_rename", { args: { repo, old, new: next, force } });
}

export function branchDelete(
  repo: RepoId,
  names: string[],
  force = false,
  sender: SenderTag = "dialogue",
): Promise<unknown> {
  return invoke<unknown>("branch_delete", { args: { repo, names, force, sender } });
}

/* ---- working copy -------------------------------------------------------- */

export function wcPathsDiscard(
  repo: RepoId,
  paths: string[],
  opts: DiscardOptions = {},
): Promise<StatusDelta> {
  return invoke<StatusDelta>("wc_paths_discard", { args: { repo, paths, opts } });
}

export function wcEntryOpen(repo: RepoId, path: string, target: OpenTarget): Promise<void> {
  return invoke<void>("wc_entry_open", { args: { repo, path, target } });
}

/* ---- diff ---------------------------------------------------------------- */

export function wcDiffLoad(
  repo: RepoId,
  path: string | null,
  perspective: DiffPerspective,
  opts: DiffOptionsWire = {},
): Promise<DiffDocument> {
  return invoke<DiffDocument>("wc_diff_load", { args: { repo, path, perspective, opts } });
}

export function wcChunkApply(
  repo: RepoId,
  hunk: HunkRef,
  direction: ChunkDirection,
  contextLines?: number,
): Promise<StatusDelta> {
  return invoke<StatusDelta>("wc_chunk_apply", {
    args: { repo, hunk, direction, contextLines },
  });
}

export function wcChunkDiscard(repo: RepoId, hunk: HunkRef, contextLines?: number): Promise<StatusDelta> {
  return invoke<StatusDelta>("wc_chunk_discard", { args: { repo, hunk, contextLines } });
}

export function wcImageDataLoad(repo: RepoId, path: string, side: ImageSide): Promise<ImageData> {
  return invoke<ImageData>("wc_image_data_load", { args: { repo, path, side } });
}

/* ---- history detail ------------------------------------------------------ */

export function historyCommitLoad(repo: RepoId, oid: string): Promise<CommitDetail> {
  return invoke<CommitDetail>("history_commit_load", { args: { repo, oid } });
}

export function historyChangesetLoad(
  repo: RepoId,
  oid: string,
  detectRenames = true,
): Promise<Changeset> {
  return invoke<Changeset>("history_changeset_load", {
    args: { repo, oid, opts: { detectRenames } },
  });
}

export function historyGraphCompute(repo: RepoId, oids: string[]): Promise<GraphLayout> {
  return invoke<GraphLayout>("history_graph_compute", { args: { repo, oids } });
}

/* ---- commits ------------------------------------------------------------- */

export function commitCreate(
  repo: RepoId,
  message: string,
  opts: CommitOptions = {},
): Promise<CommitResult> {
  return invoke<CommitResult>("commit_create", { args: { repo, message, opts } });
}

export function commitAmend(
  repo: RepoId,
  message: string | null,
  opts: CommitOptions = {},
): Promise<CommitResult> {
  return invoke<CommitResult>("commit_amend", { args: { repo, message, opts } });
}

/* ---- remotes ------------------------------------------------------------- */

export function repositoryRemotesLoad(repo: RepoId): Promise<RepositoryRemotes> {
  return invoke<RepositoryRemotes>("repository_remotes_load", { args: { repo } });
}

export function remoteFetch(
  repo: RepoId,
  remote: string,
  refspecs: string[] = [],
  mode: RemoteFetchMode = "default",
): Promise<unknown> {
  return invoke<unknown>("remote_fetch", { args: { repo, remote, refspecs, mode } });
}

export function remotePull(
  repo: RepoId,
  remote: string,
  branch: string | null = null,
  opts: PullOptions = {},
): Promise<unknown> {
  return invoke<unknown>("remote_pull", { args: { repo, remote, branch, opts } });
}

export function remotePublishBranch(
  repo: RepoId,
  remote: string,
  branch: string,
  setUpstream = false,
): Promise<unknown> {
  return invoke<unknown>("remote_publish_branch", {
    args: { repo, remote, branch, setUpstream },
  });
}

/* ---- integrate ----------------------------------------------------------- */

export function mergeStart(
  repo: RepoId,
  revision: Revision,
  opts: MergeOptions = {},
): Promise<IntegrationResult> {
  return invoke<IntegrationResult>("merge_start", { args: { repo, revision, opts } });
}

export function mergeAbort(repo: RepoId): Promise<StatusDelta> {
  return invoke<StatusDelta>("merge_abort", { args: { repo } });
}

export function rebaseStart(
  repo: RepoId,
  upstream: Revision,
  onto: Revision | null = null,
  opts: RebaseOptions = {},
): Promise<IntegrationResult> {
  return invoke<IntegrationResult>("rebase_start", { args: { repo, upstream, onto, opts } });
}

export function integrationContinue(repo: RepoId, message: string | null = null): Promise<IntegrationResult> {
  return invoke<IntegrationResult>("integration_continue", { args: { repo, message } });
}

export function integrationSkip(repo: RepoId): Promise<IntegrationResult> {
  return invoke<IntegrationResult>("integration_skip", { args: { repo } });
}

export function mergeToolInvoke(repo: RepoId, paths: string[]): Promise<unknown> {
  return invoke<unknown>("merge_tool_invoke", { args: { repo, paths } });
}

export function wcMergeStatusLoad(repo: RepoId, revision: Revision | null = null): Promise<MergeStatus> {
  return invoke<MergeStatus>("wc_merge_status_load", { args: { repo, revision } });
}

/* ---- stash --------------------------------------------------------------- */

export function stashSave(repo: RepoId, opts: StashSaveOptions = {}): Promise<unknown> {
  return invoke<unknown>("stash_save", { args: { repo, opts } });
}

/* ---- shared -------------------------------------------------------------- */

/** `history_contributors_load` — retained for the compare/summary surfaces. */
export function historyContributorsLoad(repo: RepoId): Promise<unknown> {
  return invoke<unknown>("history_contributors_load", { args: { repo } });
}

/* ---- application --------------------------------------------------------- */
/* The only family with no `repo` argument. See `bindings.ts` for why.        */

export function appInfoLoad(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info_load");
}

export function appReleaseNotesLoad(locale?: string | null): Promise<ReleaseNotes> {
  return invoke<ReleaseNotes>("app_release_notes_load", { args: { locale: locale ?? null } });
}

export function appUpdateCheck(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("app_update_check");
}

/**
 * Downloads and installs the pending update, reporting progress on a channel.
 *
 * The channel is created here rather than by the caller so the `Channel`
 * import stays in this file, which is meant to be the only call site of
 * `invoke`. On macOS the process is replaced on disk and `readyToRestart`
 * arrives last — the app keeps running the *old* code until it relaunches, so
 * the caller must offer a restart rather than assuming the new version is live.
 */
export function appUpdateInstall(
  onProgress: (progress: UpdateProgress) => void,
): Promise<void> {
  const channel = new Channel<UpdateProgress>();
  channel.onmessage = onProgress;
  return invoke<void>("app_update_install", { onProgress: channel });
}

export function appFeedbackCompose(request: FeedbackRequest): Promise<FeedbackTarget> {
  return invoke<FeedbackTarget>("app_feedback_compose", { args: request });
}

/**
 * Restarts the process.
 *
 * Only meaningful after `appUpdateInstall` has reached `readyToRestart`: macOS
 * swaps the bundle on disk and leaves the running process on the old code, so
 * the new version is not live until this returns the app. It never resolves in
 * the old process — the frontend calls it and stops caring.
 */
export function relaunch(): Promise<void> {
  return invoke<void>("app_relaunch");
}

export type { AppInfo, FeedbackTarget, ReleaseNotes, UpdateCheck, UpdateProgress, UpdateStatus };

export type { UpstreamTracking };
