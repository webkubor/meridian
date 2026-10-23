/**
 * Handlers for every wire command, browser mock mode.
 *
 * Each handler mirrors the argument shape its `commands.ts` wrapper sends and
 * the payload its Rust counterpart returns (camelCase wire). Mutations update
 * `state.ts` and emit the app events the real backend would, so composables
 * refresh exactly as they do against libgit2.
 */

import type {
  AppInfo,
  BranchInfo,
  Changeset,
  CheckoutResult,
  CommitDetail,
  CommitResult,
  CommitPage,
  CompareSummary,
  CrashNote,
  DebugState,
  DiffDocument,
  FeedbackTarget,
  GraphLayout,
  ImageData,
  IntegrationResult,
  MergeStatus,
  ReleaseNotes,
  RepoInfo,
  RepoRoot,
  RepositoryRemotes,
  StatusDelta,
  UpdateCheck,
} from "../bindings";

import {
  changesetFor,
  commitDetail,
  diffFor,
  graphLayout,
  imageData,
  RELEASE_NOTES_EN,
  RELEASE_NOTES_ZH,
  statusDelta,
  toCommitInfo,
  toEntryStatuses,
  DIAGNOSTICS_TEXT,
} from "./fixtures";
import {
  emitMockEvent,
  mockError,
  MOCK_REPO_ID,
  mockState,
} from "./state";

type Args = Record<string, unknown>;

const str = (args: Args, key: string): string => String(args[key]);
const strList = (args: Args, key: string): string[] =>
  Array.isArray(args[key]) ? (args[key] as string[]) : [];
const optStr = (args: Args, key: string): string | undefined =>
  args[key] == null ? undefined : String(args[key]);
const optBool = (args: Args, key: string): boolean | undefined =>
  args[key] == null ? undefined : Boolean(args[key]);
const optRecord = (args: Args, key: string): Record<string, unknown> =>
  args[key] && typeof args[key] === "object" ? (args[key] as Record<string, unknown>) : {};

function repoInfo(at: number | null = null): RepoInfo {
  const repo = at === null ? mockState().repos[0]! : mockState().repos[at]!;
  return {
    id: repo.id,
    gitdir: repo.gitdir,
    workdir: repo.workdir,
    isBare: repo.isBare,
    isEmpty: repo.isEmpty,
    headBranch: repo.headOid ? repo.headBranch : null,
    headOid: repo.headOid,
    state: repo.state as RepoInfo["state"],
    discovered: repo.discovered,
  };
}

function currentTipOid(): string | null {
  return mockState().repos[0]?.headOid ?? null;
}

function statusLoad(): StatusDelta {
  return statusDelta(MOCK_REPO_ID, toEntryStatuses(mockState().entries));
}

function refetchAfterMutation(repoId: string, kind: "refsChanged" | "workingCopyChanged") {
  emitMockEvent({ kind, payload: { repo: repoId } });
}

function findCommit(oid: string) {
  const commits = mockState().commits;
  const exact = commits.find((c) => c.oid === oid || c.shortOid === oid);
  if (exact) return exact;
  return commits.find((c) => c.oid.startsWith(oid) || c.shortOid.startsWith(oid));
}

function commitPage(limit: number, skip: number): CommitPage {
  const commits = mockState().commits;
  const page = commits.slice(skip, skip + limit).map(toCommitInfo);
  return { commits: page, truncated: skip + limit < commits.length };
}

function branchInfos(): BranchInfo[] {
  const state = mockState();
  const headBranch = state.repos[0]?.headBranch ?? null;
  const tip = state.commits[state.commits.length - 1];
  const summaries = new Map<string, string>();
  for (const c of state.commits) summaries.set(c.oid, c.summary);
  return state.branches.map((branch) => ({
    name: branch.name,
    fullName: branch.fullName,
    oid: branch.oid,
    isHead: branch.name === headBranch,
    isRemote: branch.isRemote,
    upstream: branch.upstream,
    ahead: branch.ahead,
    behind: branch.behind,
    tipSummary: summaries.get(branch.oid) ?? tip?.summary ?? "",
    tipTime: tip?.committer.time ?? 0,
  }));
}

function makeCommitOid(seed: number): string {
  let out = "f";
  let x = seed + 0x5f3759df;
  for (let i = 0; i < 5; i += 1) {
    x = (Math.imul(x ^ (x >>> 13), 0x5bd1e995) >>> 0) + seed;
    out += (x >>> 0).toString(16).padStart(8, "0");
  }
  return out.slice(0, 40);
}

function commitResult(message: string, amend: boolean, parentCount: number): CommitResult {
  const state = mockState();
  const repo = state.repos[0]!;
  const oid = makeCommitOid(Date.now() % 100000);
  const now = Math.floor(Date.now() / 1000);
  const summary = message.split("\n")[0] ?? "(empty)";
  return {
    oid,
    shortOid: oid.slice(0, 7),
    parents: amend && repo.headOid ? [] : repo.headOid ? [repo.headOid] : [],
    parentCount,
    summary,
    message,
    author: { name: "Web Kubor", email: "webkubor@163.com", time: now },
    committer: { name: "Web Kubor", email: "webkubor@163.com", time: now },
    branch: repo.headBranch ?? undefined,
    detached: !repo.headBranch,
    amend,
  };
}

function nextCommit(message: string): { oid: string; shortOid: string } {
  const state = mockState();
  const repo = state.repos[0]!;
  const oid = makeCommitOid(Date.now() % 999983);
  const now = Math.floor(Date.now() / 1000);
  const summary = message.split("\n")[0] ?? "(empty)";
  const signature = { name: "Web Kubor", email: "webkubor@163.com", time: now };
  state.commits.push({
    oid,
    shortOid: oid.slice(0, 7),
    parents: repo.headOid ? [repo.headOid] : [],
    summary,
    message,
    author: signature,
    committer: signature,
    decorations: repo.headBranch ? [repo.headBranch] : [],
    lane: "main",
  });
  repo.headOid = oid;
  if (repo.headBranch) {
    const branch = state.branches.find((b) => b.name === repo.headBranch);
    if (branch) branch.oid = oid;
  }
  return { oid, shortOid: oid.slice(0, 7) };
}

function debugState(): DebugState {
  const state = mockState();
  return {
    enabled: state.debugEnabled,
    source: state.debugEnabled ? "setting" : "off",
    logDir: "/tmp/meridian-mock/logs",
    logFile: "/tmp/meridian-mock/logs/meridian.log",
    level: "debug",
    devtoolsReady: true,
  };
}

function appInfo(): AppInfo {
  return {
    name: "Meridian",
    version: "0.1.0-mock",
    commit: "mock0001",
    buildProfile: "debug",
    rustVersion: "mock (browser)",
    osName: "Browser",
    osVersion: navigator.userAgent.slice(0, 40),
    arch: "wasm32",
    repoSlug: "webkubor/meridian",
    updatesConfigured: true,
    canSelfUpdate: false,
    diagnostics: DIAGNOSTICS_TEXT,
  };
}

/** The full registry — one handler per wire command. */
export const handlers: Record<string, (args: Args) => unknown> = {
  /* ---------------- diagnostics ---------------- */
  diagnostics_log_tail: (args) =>
    mockState().logLines.slice(-Number(args.lines ?? 200)).join("\n"),
  diagnostics_log_write: (args) => {
    const line = `${new Date().toISOString()} ${String(args.level ?? "info").toUpperCase()} ${String(args.source ?? "ui")} — ${String(args.message ?? "")}`;
    mockState().logLines.push(line);
    return undefined;
  },
  diagnostics_debug_get: () => debugState(),
  diagnostics_debug_set: (args) => {
    mockState().debugEnabled = Boolean(args.enabled);
    return debugState();
  },
  diagnostics_reveal_logs: () => undefined,
  diagnostics_export: () => ({
    path: "/tmp/meridian-mock/diagnostics-export.txt",
    bytes: DIAGNOSTICS_TEXT.length,
  }),
  diagnostics_open_devtools: () => undefined,
  diagnostics_last_crash: (): CrashNote | null => mockState().crashNote,
  diagnostics_dismiss_crash: () => {
    mockState().crashNote = null;
    return undefined;
  },

  /* ---------------- repository ---------------- */
  repository_open: (args) => {
    const path = str(args, "path");
    const state = mockState();
    const existing = state.repos.find((r) => r.workdir === path || r.id === path);
    if (existing) return repoInfo();
    const repo = {
      id: `${path}/.git`,
      gitdir: `${path}/.git`,
      workdir: path,
      headBranch: "main",
      headOid: currentTipOid(),
      state: "clean",
      isEmpty: false,
      isBare: false,
      discovered: false,
    };
    state.repos.push(repo);
    emitMockEvent({ kind: "reposChanged" });
    return repoInfo(state.repos.length - 1);
  },
  repository_refresh: () => repoInfo(),
  repository_init: (args) => {
    const path = str(args, "path");
    const state = mockState();
    state.repos = [
      {
        id: `${path}/.git`,
        gitdir: `${path}/.git`,
        workdir: path,
        headBranch: optStr(args, "initialBranch") ?? "main",
        headOid: null,
        state: "clean",
        isEmpty: true,
        isBare: Boolean(args.bare),
        discovered: false,
      },
    ];
    state.commits = [];
    state.branches = [];
    state.entries = [];
    emitMockEvent({ kind: "reposChanged" });
    return repoInfo();
  },
  repository_clone: (args) => {
    const destination = str(args, "destination") || "/tmp/meridian-mock/clone";
    const state = mockState();
    state.repos = [
      {
        id: `${destination}/.git`,
        gitdir: `${destination}/.git`,
        workdir: destination,
        headBranch: optStr(args, "initialBranch") ?? "main",
        headOid: currentTipOid(),
        state: "clean",
        isEmpty: false,
        isBare: Boolean(args.bare),
        discovered: false,
      },
    ];
    emitMockEvent({ kind: "reposChanged" });
    return repoInfo();
  },
  repository_clone_suggest_destination: (args) => {
    const url = String(args.url ?? "");
    const name = url.split("/").pop()?.replace(/\.git$/, "") || "clone";
    return `/Users/webkubor/dev/clients/${name}`;
  },
  repository_discover: () => [
    {
      name: "meridian",
      path: "/Users/webkubor/dev/clients/meridian",
      gitdir: `${MOCK_REPO_ID}`,
      workdir: "/Users/webkubor/dev/clients/meridian",
      isBare: false,
      isWorktree: false,
    },
    {
      name: "dotfiles",
      path: "/Users/webkubor/dev/dotfiles",
      gitdir: "/Users/webkubor/dev/dotfiles/.git",
      workdir: "/Users/webkubor/dev/dotfiles",
      isBare: false,
      isWorktree: false,
    },
  ] satisfies RepoRoot[],

  /* ---------------- working copy ---------------- */
  wc_status_load: () => statusLoad(),
  wc_stage_paths: (args) => {
    const paths = strList(args, "paths");
    for (const entry of mockState().entries) {
      if (paths.includes(entry.path)) {
        if (entry.code === "untracked") entry.code = "added";
        entry.staged = true;
      }
    }
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return statusLoad();
  },
  wc_unstage_paths: (args) => {
    const paths = strList(args, "paths");
    for (const entry of mockState().entries) {
      if (paths.includes(entry.path)) {
        if (entry.code === "added") entry.code = "untracked";
        entry.staged = false;
      }
    }
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return statusLoad();
  },
  wc_entry_assume_unchanged_set: (args) => {
    const paths = strList(args, "paths");
    const value = Boolean(args.value);
    for (const entry of mockState().entries) {
      if (paths.includes(entry.path)) entry.assumeUnchanged = value;
    }
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return statusLoad();
  },
  wc_paths_discard: (args) => {
    const paths = strList(args, "paths");
    const state = mockState();
    state.entries = state.entries.filter((entry) => !paths.includes(entry.path));
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return statusLoad();
  },
  wc_entry_open: () => undefined,
  wc_diff_load: (args) => {
    const path = str(args, "path");
    const perspective = str(args, "perspective");
    const entry = mockState().entries.find((e) => e.path === path);
    if (!entry) {
      return {
        path,
        perspective: perspective as DiffDocument["perspective"],
        isBinary: false,
        hunks: [],
        additions: 0,
        deletions: 0,
        truncated: false,
      } satisfies DiffDocument;
    }
    return diffFor(path, entry.code, perspective);
  },
  wc_chunk_apply: (args) => {
    const hunk = optRecord(args, "hunk");
    const path = String(hunk.path ?? "");
    for (const entry of mockState().entries) {
      if (entry.path === path) entry.staged = true;
    }
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return statusLoad();
  },
  wc_chunk_discard: (args) => {
    const hunk = optRecord(args, "hunk");
    const path = String(hunk.path ?? "");
    const state = mockState();
    state.entries = state.entries.filter((entry) => entry.path !== path);
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return statusLoad();
  },
  wc_image_data_load: (args) =>
    imageData(str(args, "path"), args.side === "old" ? "old" : "new") as ImageData,

  /* ---------------- history ---------------- */
  history_commits_load: (args) => {
    // commands.ts spreads HistoryCommitsOptions into the args top level.
    const limit = Number(args.limit ?? 200);
    const skip = Number(args.skip ?? 0);
    const search = optStr(args, "search");
    let page = commitPage(limit, skip);
    if (search) {
      const needle = search.toLowerCase();
      const mode = String(args.searchMode ?? "all");
      const commits = mockState().commits.filter((c) => {
        if (mode === "all" || mode === "message") return c.message.toLowerCase().includes(needle);
        if (mode === "author") {
          return c.author.name.toLowerCase().includes(needle) || c.author.email.toLowerCase().includes(needle);
        }
        return c.oid.toLowerCase().includes(needle);
      });
      page = {
        commits: commits.slice(skip, skip + limit).map(toCommitInfo),
        truncated: skip + limit < commits.length,
      };
    }
    return page;
  },
  history_commit_load: (args) => {
    const commit = findCommit(str(args, "oid"));
    if (!commit) mockError("unknown_revision", `No commit ${String(args.oid)}`);
    return commitDetail(commit) as CommitDetail;
  },
  history_changeset_load: (args) => {
    const commit = findCommit(str(args, "oid"));
    if (!commit) mockError("unknown_revision", `No commit ${String(args.oid)}`);
    return changesetFor(commit) as Changeset;
  },
  history_merge_base: () => {
    const commits = mockState().commits;
    const main = commits[commits.length - 2] ?? commits[0];
    return {
      mergeBase: main?.oid ?? null,
      ahead: 1,
      behind: 0,
    } satisfies CompareSummary;
  },
  history_graph_compute: (args) => {
    const oids = strList(args, "oids");
    return graphLayout(mockState().commits, oids) as GraphLayout;
  },
  history_contributors_load: () => [
    { name: "Web Kubor", email: "webkubor@163.com", commits: 96 },
    { name: "Ada Lin", email: "ada@example.com", commits: 28 },
    { name: "Mori Tanaka", email: "mori@example.com", commits: 11 },
    { name: "Jules Verne", email: "jules@example.com", commits: 7 },
  ],

  /* ---------------- branches ---------------- */
  branch_list: () => branchInfos(),
  branch_create: (args) => {
    const state = mockState();
    const name = str(args, "name");
    const start = optStr(args, "start") ?? state.repos[0]?.headOid ?? "";
    const commit = findCommit(start) ?? state.commits[state.commits.length - 1]!;
    const branch = {
      name,
      fullName: `refs/heads/${name}`,
      oid: commit.oid,
      isRemote: false,
      ahead: 0,
      behind: 0,
    };
    state.branches.push(branch);
    if (args.checkout) {
      state.repos[0]!.headBranch = name;
      state.repos[0]!.headOid = commit.oid;
    }
    refetchAfterMutation(str(args, "repo"), "refsChanged");
    return branchInfos().find((b) => b.name === name) ?? branchInfos()[0]!;
  },
  branch_checkout: (args) => {
    const state = mockState();
    const name = str(args, "name");
    const branch = state.branches.find((b) => b.name === name);
    if (!branch) mockError("reference_not_found", `No branch ${name}`);
    const detach = optBool(optRecord(args, "opts"), "detach");
    state.repos[0]!.headBranch = detach ? null : name;
    state.repos[0]!.headOid = branch.oid;
    refetchAfterMutation(str(args, "repo"), "refsChanged");
    return {
      branch: detach ? null : name,
      oid: branch.oid,
      detached: Boolean(detach),
    } satisfies CheckoutResult;
  },
  branch_checkout_remote: (args) => {
    const state = mockState();
    const remoteBranch = str(args, "remoteBranch");
    const localName = optStr(args, "localName") ?? remoteBranch.split("/").pop()!;
    const source = state.branches.find((b) => b.fullName === `refs/remotes/${remoteBranch}`);
    const branch = {
      name: localName,
      fullName: `refs/heads/${localName}`,
      oid: source?.oid ?? currentTipOid() ?? "",
      isRemote: false,
      upstream: remoteBranch,
      ahead: 0,
      behind: 0,
    };
    state.branches.push(branch);
    state.repos[0]!.headBranch = localName;
    state.repos[0]!.headOid = branch.oid;
    refetchAfterMutation(str(args, "repo"), "refsChanged");
    return branchInfos().find((b) => b.name === localName) ?? branchInfos()[0]!;
  },
  branch_rename: (args) => {
    const state = mockState();
    const oldName = str(args, "old");
    const next = str(args, "new");
    const branch = state.branches.find((b) => b.name === oldName);
    if (!branch) mockError("reference_not_found", `No branch ${oldName}`);
    branch.name = next;
    branch.fullName = `refs/heads/${next}`;
    if (state.repos[0]!.headBranch === oldName) state.repos[0]!.headBranch = next;
    refetchAfterMutation(str(args, "repo"), "refsChanged");
    return branchInfos().find((b) => b.name === next) ?? branchInfos()[0]!;
  },
  branch_delete: (args) => {
    const names = strList(args, "names");
    const state = mockState();
    state.branches = state.branches.filter(
      (b) => !names.includes(b.name) || b.name === state.repos[0]?.headBranch,
    );
    refetchAfterMutation(str(args, "repo"), "refsChanged");
    return undefined;
  },

  /* ---------------- commits ---------------- */
  commit_create: (args) => {
    const message = str(args, "message");
    const state = mockState();
    if (!message.trim()) mockError("invalid_request", "Commit message is empty", "internal");
    nextCommit(message);
    const stagedPaths = new Set(
      state.entries.filter((e) => e.staged && e.code !== "untracked" && e.code !== "ignored").map((e) => e.path),
    );
    state.entries = state.entries.filter(
      (e) => !(stagedPaths.has(e.path) && e.staged),
    );
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return commitResult(message, false, 1) as CommitResult;
  },
  commit_amend: (args) => {
    const message = str(args, "message");
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return commitResult(message, true, 1) as CommitResult;
  },

  /* ---------------- remotes ---------------- */
  repository_remotes_load: () => {
    const state = mockState();
    return {
      remotes: state.remotes.map((remote) => ({
        name: remote.name,
        fetchUrl: remote.fetchUrl,
        pushUrl: remote.pushUrl,
        fetchRefspecs: [...remote.fetchRefspecs],
        pushRefspecs: [...remote.pushRefspecs],
        fetchMode: "default" as const,
        trackedBranches: [...remote.trackedBranches],
      })),
      upstreams: state.branches
        .filter((b) => !b.isRemote)
        .map((b) => ({
          branch: b.name,
          remote: b.upstream ? b.upstream.split("/")[0]! : null,
          remoteBranch: b.upstream ?? null,
          mergeRef: b.upstream ? `refs/remotes/${b.upstream}` : null,
          ahead: b.ahead,
          behind: b.behind,
          gone: false,
        })),
    } satisfies RepositoryRemotes;
  },
  remote_fetch: () => ({
    updatedRefs: ["refs/remotes/origin/main"],
  }),
  remote_pull: () => ({ commits: 1, mergeRequired: false }),
  remote_publish_branch: () => undefined,

  /* ---------------- integrate ---------------- */
  merge_start: (args) => {
    const revision = optStr(args, "revision") ?? "origin/main";
    const state = mockState();
    if (state.entries.some((e) => e.code === "unmerged")) {
      return {
        outcome: { kind: "conflicted" },
        conflicts: [{ path: "src/engine/merge.rs" }],
        notes: [`Merge of ${revision} left one conflict`],
      } satisfies IntegrationResult;
    }
    refetchAfterMutation(str(args, "repo"), "refsChanged");
    return {
      outcome: { kind: "merged", commit: currentTipOid() ?? "", parents: [] },
      conflicts: [],
      notes: [],
    } satisfies IntegrationResult;
  },
  merge_abort: () => {
    const state = mockState();
    for (const entry of state.entries) {
      if (entry.code === "unmerged") entry.code = "modified";
    }
    refetchAfterMutation(MOCK_REPO_ID, "workingCopyChanged");
    return statusLoad();
  },
  rebase_start: () => ({
    outcome: { kind: "rebaseFinished", head: currentTipOid() ?? "", applied: 3, dropped: 0 },
    conflicts: [],
    notes: [],
  }) satisfies IntegrationResult,
  integration_continue: () => ({
    outcome: { kind: "rebaseFinished", head: currentTipOid() ?? "", applied: 1, dropped: 0 },
    conflicts: [],
    notes: [],
  }) satisfies IntegrationResult,
  integration_skip: () => ({
    outcome: { kind: "rebaseFinished", head: currentTipOid() ?? "", applied: 1, dropped: 1 },
    conflicts: [],
    notes: [],
  }) satisfies IntegrationResult,
  merge_tool_invoke: () => undefined,
  wc_merge_status_load: (args) => {
    const revision = optStr(args, "revision") ?? "origin/main";
    return {
      state: "mergeable",
      incoming: revision,
      base: currentTipOid() ?? undefined,
      fastForward: false,
      conflictPreview: [],
    } satisfies MergeStatus;
  },

  /* ---------------- stash ---------------- */
  stash_save: (args) => {
    const opts = optRecord(args, "opts");
    const state = mockState();
    const now = Math.floor(Date.now() / 1000);
    const result = {
      index: 0,
      message: optStr(opts, "message") ?? "WIP on main",
      date: now,
      author: { name: "Web Kubor", email: "webkubor@163.com", time: now },
    };
    state.entries = [];
    refetchAfterMutation(str(args, "repo"), "workingCopyChanged");
    return result;
  },

  /* ---------------- app ---------------- */
  app_info_load: () => appInfo(),
  app_release_notes_load: (args) => {
    const locale = String(args.locale ?? "en");
    const zh = locale.startsWith("zh");
    return {
      markdown: zh ? RELEASE_NOTES_ZH : RELEASE_NOTES_EN,
      locale: zh ? "zh_cn" : "en",
      currentVersion: "0.1.0-mock",
    } satisfies ReleaseNotes;
  },
  app_update_check: (): UpdateCheck => {
    const state = mockState();
    if (state.updateInstalled) {
      return {
        kind: "checked",
        status: {
          currentVersion: "0.1.0-mock",
          latestVersion: "0.1.0-mock",
          notes: null,
          publishedAt: new Date().toISOString(),
          downloadBytes: null,
        },
      };
    }
    return {
      kind: "checked",
      status: {
        currentVersion: "0.1.0-mock",
        latestVersion: "0.2.0",
        notes: "## 0.2.0\n\n- Browser mock mode for UI iteration.\n",
        publishedAt: "2026-02-01T00:00:00Z",
        downloadBytes: 62_400_000,
      },
    };
  },
  app_update_install: (args) => {
    const channel = args.onProgress as { onmessage?: ((frame: unknown) => void) | null } | undefined;
    const total = 62_400_000;
    const frames: Array<Record<string, unknown>> = [];
    for (let step = 1; step <= 5; step += 1) {
      frames.push({ phase: "downloading", received: Math.round((total * step) / 5), total });
    }
    frames.push({ phase: "downloaded" });
    frames.push({ phase: "installing" });
    frames.push({ phase: "readyToRestart" });
    queueMicrotask(() => {
      for (const frame of frames) channel?.onmessage?.(frame);
    });
    mockState().updateInstalled = true;
    return undefined;
  },
  app_feedback_compose: (args) => {
    const kind = String(args.kind ?? "bug");
    const summary = optStr(args, "summary");
    const url = new URL(
      `https://github.com/webkubor/meridian/issues/new`,
    );
    url.searchParams.set("template", kind === "feature" ? "feature_request.md" : "bug_report.md");
    if (summary) url.searchParams.set("title", summary);
    const context = optStr(args, "context");
    const prefilled = context ? context.split("\n").slice(0, 6) : [];
    return { url: url.toString(), prefilled } satisfies FeedbackTarget;
  },
  app_relaunch: () => undefined,
  app_pick_directory: () => "/Users/webkubor/dev/clients",
};
