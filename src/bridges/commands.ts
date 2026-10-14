/**
 * The only call site of `invoke` in the app (docs/02 §13): stores and
 * composables go through these typed wrappers, so docs/03 stays authoritative.
 *
 * Tauri maps the `args` object onto each command's single argument struct —
 * `invoke("branch_create", { args: { repo, name, start, checkout, sender } })`.
 */

import { Channel, invoke as tauriInvoke } from "@tauri-apps/api/core";

import type { DebugState, CrashNote } from "./bindings";
import { captureFailure, normalizeError } from "../diagnostics/failures";
import { recordCall, safeArgumentSummary } from "../diagnostics/recorder";

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
  CompareSummary,
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
  RemoteFetchOptions,
  RepoId,
  RepoInfo,
  RepoRoot,
  RepositoryInitOptions,
  RepositoryCloneOptions,
  RepositoryDiscoverOptions,
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

/**
 * The single instrumented path to the backend (docs/02 §13).
 *
 * Every wrapper below goes through it so that a failure is recorded, written to
 * the local log and handed to the failure net exactly once — and so the caller
 * still receives the *same* rejection the backend sent. A diagnostics command
 * that fails is recorded but never logged, which is what stops the reporter
 * from reporting itself in a loop.
 */
export async function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  const startedAt = new Date().toISOString();
  const started = performance.now();
  const summary = safeArgumentSummary(args, name);
  try {
    const value = await tauriInvoke<T>(name, args);
    recordCall({ command: name, startedAt, durationMs: Math.round(performance.now() - started), ok: true, ...summary });
    return value;
  } catch (error) {
    const payload = normalizeError(error);
    recordCall({ command: name, startedAt, durationMs: Math.round(performance.now() - started), ok: false, errorCode: payload.code, errorDomain: payload.domain, ...summary });
    captureFailure(error, name, { newCall: true, payload });
    throw error;
  }
}

export function diagnosticsLogTail(lines: number): Promise<string> {
  return call<string>("diagnostics_log_tail", { args: { lines } });
}
export function diagnosticsLogWrite(level: "error" | "warn" | "info" | "debug", message: string, source?: string): Promise<void> {
  return call<void>("diagnostics_log_write", { args: { level, message, source } });
}
export function diagnosticsDebugGet(): Promise<DebugState> {
  return call<DebugState>("diagnostics_debug_get");
}
export function diagnosticsDebugSet(enabled: boolean): Promise<DebugState> {
  return call<DebugState>("diagnostics_debug_set", { args: { enabled } });
}
export function diagnosticsRevealLogs(): Promise<void> {
  return call<void>("diagnostics_reveal_logs");
}
export function diagnosticsExport(context?: string): Promise<{ path: string; bytes: number }> {
  return call<{ path: string; bytes: number }>("diagnostics_export", { args: { context } });
}
export function diagnosticsOpenDevtools(): Promise<void> {
  return call<void>("diagnostics_open_devtools");
}
export function diagnosticsLastCrash(): Promise<CrashNote | null> {
  return call<CrashNote | null>("diagnostics_last_crash");
}
export function diagnosticsDismissCrash(): Promise<void> {
  return call<void>("diagnostics_dismiss_crash");
}

export function repositoryOpen(path: string, sender: SenderTag = "dialogue"): Promise<RepoInfo> {
  return call<RepoInfo>("repository_open", { args: { path, sender } });
}

/** Re-read HEAD and integration state for an open repository after ref changes. */
export function repositoryRefresh(repo: RepoId): Promise<RepoInfo> {
  return call<RepoInfo>("repository_refresh", { args: { repo } });
}

export function repositoryInit(
  path: string,
  options: RepositoryInitOptions = {},
): Promise<RepoInfo> {
  return call<RepoInfo>("repository_init", { args: { path, ...options } });
}

/** Clones `url` and opens the result. Resolves once the transfer is done. */
export function repositoryClone(url: string, options: RepositoryCloneOptions = {}): Promise<RepoInfo> {
  return call<RepoInfo>("repository_clone", {
    args: { url, destination: "", ...options },
  });
}

/**
 * The folder name a clone URL implies, computed by the same code that will
 * perform the clone.
 *
 * Asked of Rust rather than reimplemented here because a suggestion the UI
 * recomputes separately is a suggestion that eventually disagrees with the one
 * the command acts on.
 */
export function repositoryCloneDestination(url: string): Promise<string> {
  return call<string>("repository_clone_suggest_destination", { url });
}

/**
 * Bounded walk for "found nearby". Returns roots, not open repositories —
 * opening one is a separate step, so scanning a home directory never silently
 * adds twenty repositories to the sidebar.
 */
export function repositoryDiscover(
  root: string,
  options: RepositoryDiscoverOptions = {},
): Promise<RepoRoot[]> {
  return call<RepoRoot[]>("repository_discover", {
    args: { root, ...options },
  });
}

export function wcStatusLoad(repo: RepoId, opts: WcStatusOptions = {}): Promise<StatusDelta> {
  return call<StatusDelta>("wc_status_load", { args: { repo, opts } });
}

export function wcStagePaths(repo: RepoId, paths: string[]): Promise<StatusDelta> {
  return call<StatusDelta>("wc_stage_paths", { args: { repo, paths } });
}

export function wcUnstagePaths(repo: RepoId, paths: string[]): Promise<StatusDelta> {
  return call<StatusDelta>("wc_unstage_paths", { args: { repo, paths } });
}

/** Set or clear git's assume-unchanged bit on paths. */
export function wcEntryAssumeUnchangedSet(
  repo: RepoId,
  paths: string[],
  value: boolean,
): Promise<StatusDelta> {
  return call<StatusDelta>("wc_entry_assume_unchanged_set", { args: { repo, paths, value } });
}

export function historyCommitsLoad(
  repo: RepoId,
  options: HistoryCommitsOptions = {},
): Promise<CommitPage> {
  return call<CommitPage>("history_commits_load", { args: { repo, ...options } });
}


export function branchList(
  repo: RepoId,
  filter: BranchFilter = "local",
  search?: string,
): Promise<BranchInfo[]> {
  return call<BranchInfo[]>("branch_list", { args: { repo, filter, search } });
}

export function branchCreate(
  repo: RepoId,
  name: string,
  start: Revision = { kind: "head" },
  checkout = true,
  sender: SenderTag = "dialogue",
): Promise<BranchInfo> {
  return call<BranchInfo>("branch_create", { args: { repo, name, start, checkout, sender } });
}

export function branchCheckout(
  repo: RepoId,
  name: string,
  opts: CheckoutOptions = {},
): Promise<CheckoutResult> {
  return call<CheckoutResult>("branch_checkout", { args: { repo, name, opts } });
}

export function branchCheckoutRemote(
  repo: RepoId,
  remoteBranch: string,
  localName?: string,
  track = true,
): Promise<BranchInfo> {
  return call<BranchInfo>("branch_checkout_remote", {
    args: { repo, remoteBranch, localName, track },
  });
}

export function branchRename(
  repo: RepoId,
  old: string,
  next: string,
  force = false,
): Promise<BranchInfo> {
  return call<BranchInfo>("branch_rename", { args: { repo, old, new: next, force } });
}

export function branchDelete(
  repo: RepoId,
  names: string[],
  force = false,
  sender: SenderTag = "dialogue",
): Promise<unknown> {
  return call<unknown>("branch_delete", { args: { repo, names, force, sender } });
}

/* ---- working copy -------------------------------------------------------- */

export function wcPathsDiscard(
  repo: RepoId,
  paths: string[],
  opts: DiscardOptions = {},
): Promise<StatusDelta> {
  return call<StatusDelta>("wc_paths_discard", { args: { repo, paths, opts } });
}

export function wcEntryOpen(repo: RepoId, path: string, target: OpenTarget): Promise<void> {
  return call<void>("wc_entry_open", { args: { repo, path, target } });
}

/* ---- diff ---------------------------------------------------------------- */

export function wcDiffLoad(
  repo: RepoId,
  path: string | null,
  perspective: DiffPerspective,
  opts: DiffOptionsWire = {},
): Promise<DiffDocument> {
  return call<DiffDocument>("wc_diff_load", { args: { repo, path, perspective, opts } });
}

export function wcChunkApply(
  repo: RepoId,
  hunk: HunkRef,
  direction: ChunkDirection,
  contextLines?: number,
): Promise<StatusDelta> {
  return call<StatusDelta>("wc_chunk_apply", {
    args: { repo, hunk, direction, contextLines },
  });
}

export function wcChunkDiscard(repo: RepoId, hunk: HunkRef, contextLines?: number): Promise<StatusDelta> {
  return call<StatusDelta>("wc_chunk_discard", { args: { repo, hunk, contextLines } });
}

export function wcImageDataLoad(repo: RepoId, path: string, side: ImageSide): Promise<ImageData> {
  return call<ImageData>("wc_image_data_load", { args: { repo, path, side } });
}

/* ---- history detail ------------------------------------------------------ */

export function historyCommitLoad(repo: RepoId, oid: string): Promise<CommitDetail> {
  return call<CommitDetail>("history_commit_load", { args: { repo, oid } });
}

export function historyChangesetLoad(
  repo: RepoId,
  oid: string,
  detectRenames = true,
): Promise<Changeset> {
  return call<Changeset>("history_changeset_load", {
    args: { repo, oid, opts: { detectRenames } },
  });
}

/**
 * Exact ahead/behind counts and the fork point for two revisions.
 *
 * `mergeBase` is `null` when the two histories are unrelated — an answer, not a
 * failure, and precisely the case where the user most needs telling.
 */
export function historyCompareSummary(
  repo: RepoId,
  left: Revision,
  right: Revision,
): Promise<CompareSummary> {
  return call<CompareSummary>("history_merge_base", { args: { repo, left, right } });
}

export function historyGraphCompute(repo: RepoId, oids: string[]): Promise<GraphLayout> {
  return call<GraphLayout>("history_graph_compute", { args: { repo, oids } });
}

/* ---- commits ------------------------------------------------------------- */

export function commitCreate(
  repo: RepoId,
  message: string,
  opts: CommitOptions = {},
): Promise<CommitResult> {
  return call<CommitResult>("commit_create", { args: { repo, message, opts } });
}

export function commitAmend(
  repo: RepoId,
  message: string | null,
  opts: CommitOptions = {},
): Promise<CommitResult> {
  return call<CommitResult>("commit_amend", { args: { repo, message, opts } });
}

/* ---- remotes ------------------------------------------------------------- */

export function repositoryRemotesLoad(repo: RepoId): Promise<RepositoryRemotes> {
  return call<RepositoryRemotes>("repository_remotes_load", { args: { repo } });
}

/**
 * Fetch a remote.
 *
 * `opts` is the per-invocation policy `{ prune, tags, depth }` — the same
 * shape `RepositoryRemote.fetchMode` reports, and *not* the `all | specified`
 * enum, which describes a remote's stored refspecs rather than one fetch.
 */
export function remoteFetch(
  repo: RepoId,
  remote: string,
  refspecs: string[] = [],
  opts: RemoteFetchOptions = {},
): Promise<unknown> {
  return call<unknown>("remote_fetch", { args: { repo, remote, refspecs, mode: opts } });
}

export function remotePull(
  repo: RepoId,
  remote: string,
  branch: string | null = null,
  opts: PullOptions = {},
): Promise<unknown> {
  return call<unknown>("remote_pull", { args: { repo, remote, branch, opts } });
}

export function remotePublishBranch(
  repo: RepoId,
  remote: string,
  branch: string,
  setUpstream = false,
): Promise<unknown> {
  return call<unknown>("remote_publish_branch", {
    args: { repo, remote, branch, setUpstream },
  });
}

/* ---- integrate ----------------------------------------------------------- */

export function mergeStart(
  repo: RepoId,
  revision: Revision,
  opts: MergeOptions = {},
): Promise<IntegrationResult> {
  return call<IntegrationResult>("merge_start", { args: { repo, revision, opts } });
}

export function mergeAbort(repo: RepoId): Promise<StatusDelta> {
  return call<StatusDelta>("merge_abort", { args: { repo } });
}

export function rebaseStart(
  repo: RepoId,
  upstream: Revision,
  onto: Revision | null = null,
  opts: RebaseOptions = {},
): Promise<IntegrationResult> {
  return call<IntegrationResult>("rebase_start", { args: { repo, upstream, onto, opts } });
}

export function integrationContinue(repo: RepoId, message: string | null = null): Promise<IntegrationResult> {
  return call<IntegrationResult>("integration_continue", { args: { repo, message } });
}

export function integrationSkip(repo: RepoId): Promise<IntegrationResult> {
  return call<IntegrationResult>("integration_skip", { args: { repo } });
}

export function mergeToolInvoke(repo: RepoId, paths: string[]): Promise<unknown> {
  return call<unknown>("merge_tool_invoke", { args: { repo, paths } });
}

export function wcMergeStatusLoad(repo: RepoId, revision: Revision | null = null): Promise<MergeStatus> {
  return call<MergeStatus>("wc_merge_status_load", { args: { repo, revision } });
}

/* ---- stash --------------------------------------------------------------- */

export function stashSave(repo: RepoId, opts: StashSaveOptions = {}): Promise<unknown> {
  return call<unknown>("stash_save", { args: { repo, opts } });
}

/* ---- shared -------------------------------------------------------------- */

/** `history_contributors_load` — retained for the compare/summary surfaces. */
export function historyContributorsLoad(repo: RepoId): Promise<unknown> {
  return call<unknown>("history_contributors_load", { args: { repo } });
}

/* ---- application --------------------------------------------------------- */
/* The only family with no `repo` argument. See `bindings.ts` for why.        */

export function appInfoLoad(): Promise<AppInfo> {
  return call<AppInfo>("app_info_load");
}

export function appReleaseNotesLoad(locale?: string | null): Promise<ReleaseNotes> {
  return call<ReleaseNotes>("app_release_notes_load", { args: { locale: locale ?? null } });
}

export function appUpdateCheck(): Promise<UpdateCheck> {
  return call<UpdateCheck>("app_update_check");
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
  return call<void>("app_update_install", { onProgress: channel });
}

export function appFeedbackCompose(request: FeedbackRequest): Promise<FeedbackTarget> {
  return call<FeedbackTarget>("app_feedback_compose", { args: request });
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
  return call<void>("app_relaunch");
}

/**
 * Asks the user for a folder. Resolves to `null` when they cancel.
 *
 * Cancellation is `null` rather than an error on purpose: a picker the user
 * dismissed is a decision, not a failure, and reporting it as a failure is how
 * "Choose…" ends up showing an error toast to someone who changed their mind.
 */
export function pickDirectory(): Promise<string | null> {
  return call<string | null>("app_pick_directory");
}

export type { AppInfo, FeedbackTarget, ReleaseNotes, UpdateCheck, UpdateProgress, UpdateStatus };

export type { UpstreamTracking };
