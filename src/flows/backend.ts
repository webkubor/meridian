/**
 * 命令流测试的可编程 Tauri 后端
 * 路由：Vitest 的 src/flows 测试
 * API：模拟 @tauri-apps/api/core.invoke，不启动 Rust 或 Tauri
 */
import type {
  BranchInfo,
  CommitInfo,
  EntryStatus,
  ErrorPayload,
  RepoInfo,
  RepoRoot,
  StatusDelta,
} from "../bridges/bindings";

export interface RecordedCall {
  name: string;
  payload: Record<string, unknown> | undefined;
}

type CommandHandler = (payload: Record<string, unknown> | undefined) => unknown | Promise<unknown>;

const REPO_ID = "/tmp/meridian-flow/.git";
const REPO_PATH = "/tmp/meridian-flow";
const OID_MAIN = "a123456789012345678901234567890123456789";
const OID_FEATURE = "b123456789012345678901234567890123456789";
const OID_OLDER = "c123456789012345678901234567890123456789";

function entry(path: string, code: EntryStatus["code"], staged = false): EntryStatus {
  return {
    path,
    code,
    staged,
    assumeUnchanged: false,
    view: {
      short: code === "untracked" ? "?" : code === "added" ? "A" : "M",
      text: code,
      tone: code === "untracked" ? "muted" : "info",
    },
  };
}

function commit(oid: string, summary: string, parents: string[] = []): CommitInfo {
  const signature = { name: "Flow Fixture", email: "flow@example.invalid", time: 1_760_000_000 };
  return {
    oid,
    shortOid: oid.slice(0, 7),
    parents,
    summary,
    message: summary,
    author: signature,
    committer: signature,
    decorations: [],
  };
}

function branch(name: string, options: Partial<BranchInfo> = {}): BranchInfo {
  const isRemote = options.isRemote ?? false;
  // A remote-tracking ref is listed the way git lists it (`origin/main`), which
  // is also how the shell resolves a branch's upstream to its remote row.
  const shortName = name;
  return {
    name: shortName,
    fullName: isRemote ? `refs/remotes/${name}` : `refs/heads/${name}`,
    oid: options.oid ?? OID_MAIN,
    isHead: options.isHead ?? false,
    isRemote,
    upstream: options.upstream,
    ahead: options.ahead ?? 0,
    behind: options.behind ?? 0,
    tipSummary: options.tipSummary ?? `tip: ${shortName}`,
    tipTime: options.tipTime ?? 1_760_000_000,
  };
}

export function makeError(command: string, patch: Partial<ErrorPayload> = {}): ErrorPayload {
  return {
    code: "invalid_request",
    message: `${command} failed in the flow fixture`,
    detail: `fixture backend rejected ${command}`,
    params: {},
    retryable: false,
    domain: "git",
    ...patch,
  };
}

/** 可重置的后端状态、命令处理器、失败队列和 IPC 调用记录。 */
export class FakeBackend {
  readonly calls: RecordedCall[] = [];
  readonly handlers = new Map<string, CommandHandler>();
  private readonly failures = new Map<string, ErrorPayload[]>();
  private pickerResult: string | null = "/tmp";
  private repo: RepoInfo = this.repositoryFixture();
  private entries: EntryStatus[] = [
    entry("src/app.ts", "modified"),
    entry("notes.txt", "untracked"),
  ];
  private commits: CommitInfo[] = [
    commit(OID_MAIN, "Add fixture main branch"),
    commit(OID_FEATURE, "Implement a fixture feature", [OID_MAIN]),
  ];
  private localBranches: BranchInfo[] = [
    branch("main", { isHead: true, upstream: "origin/main", ahead: 1 }),
    branch("feature/ready", { oid: OID_FEATURE, tipSummary: "Implement fixture feature" }),
  ];
  private remoteBranches: BranchInfo[] = [
    branch("origin/main", { isRemote: true, upstream: "origin", tipSummary: "Remote main" }),
    branch("origin/review", { isRemote: true, oid: OID_FEATURE, tipSummary: "Review branch" }),
  ];

  constructor() {
    this.installDefaults();
  }

  /** Restore the open-repository fixture and clear all injected behavior. */
  reset(): void {
    this.calls.splice(0);
    this.failures.clear();
    this.handlers.clear();
    this.pickerResult = "/tmp";
    this.repo = this.repositoryFixture();
    this.entries = [entry("src/app.ts", "modified"), entry("notes.txt", "untracked")];
    this.commits = [
      commit(OID_MAIN, "Add fixture main branch"),
      commit(OID_FEATURE, "Implement a fixture feature", [OID_MAIN]),
    ];
    this.localBranches = [
      branch("main", { isHead: true, upstream: "origin/main", ahead: 1 }),
      branch("feature/ready", { oid: OID_FEATURE, tipSummary: "Implement fixture feature" }),
    ];
    this.remoteBranches = [
      branch("origin/main", { isRemote: true, upstream: "origin", tipSummary: "Remote main" }),
      branch("origin/review", { isRemote: true, oid: OID_FEATURE, tipSummary: "Review branch" }),
    ];
    this.installDefaults();
  }

  /** Replace one command implementation. */
  setHandler(name: string, handler: CommandHandler): void {
    this.handlers.set(name, handler);
  }

  /** Reject exactly the next invocation of `name`. */
  failNext(name: string, payload = makeError(name)): void {
    const queued = this.failures.get(name) ?? [];
    queued.push(payload);
    this.failures.set(name, queued);
  }

  /** Make the next folder-picker result explicit, including cancellation. */
  pickNext(result: string | null): void {
    this.pickerResult = result;
  }

  /**
   * Reject every invoked command.
   *
   * The log sink is included on purpose: the failure net does not write a
   * failure back to the log when the failing command is itself a `diagnostics_`
   * one, so a failing sink cannot recurse. Each command's message is distinct,
   * which is what the net's two-second de-duplication needs to keep counting.
   */
  rejectAll(): void {
    for (const name of this.handlers.keys()) {
      this.handlers.set(name, () => {
        throw makeError(name, { message: `${name} rejected by the meta test` });
      });
    }
    this.rejectUnknown = true;
  }

  private rejectUnknown = false;

  /** Fake `invoke`, retaining the exact command and payload received. */
  async invoke<T>(name: string, payload?: Record<string, unknown>): Promise<T> {
    this.calls.push({ name, payload });
    const failure = this.failures.get(name)?.shift();
    if (failure) throw failure;
    const handler = this.handlers.get(name);
    if (handler) return (await handler(payload)) as T;
    if (this.rejectUnknown) throw makeError(name, { message: `${name} meta-test failure` });
    return (await this.defaultResult(name, payload)) as T;
  }

  /** Select a call by command and optional occurrence. */
  call(name: string, occurrence = 0): RecordedCall | undefined {
    return this.calls.filter((call) => call.name === name)[occurrence];
  }

  private installDefaults(): void {
    this.handlers.set("repository_open", () => this.repo);
    this.handlers.set("repository_init", (payload) => {
      const args = this.commandArgs(payload);
      const path = String(args.path ?? REPO_PATH);
      this.repo = { ...this.repositoryFixture(), id: `${path}/.git`, gitdir: `${path}/.git`, workdir: path, isEmpty: true };
      this.localBranches = [branch(String(args.initialBranch ?? "main"), { isHead: true })];
      this.entries = [];
      this.commits = [];
      return this.repo;
    });
    this.handlers.set("repository_clone_suggest_destination", () => "fixture-repository");
    this.handlers.set("repository_clone", (payload) => {
      const args = this.commandArgs(payload);
      const destination = String(args.destination ?? "/tmp/fixture-repository");
      this.repo = { ...this.repositoryFixture(), id: `${destination}/.git`, gitdir: `${destination}/.git`, workdir: destination };
      return this.repo;
    });
    this.handlers.set("app_pick_directory", () => this.pickerResult);
    this.handlers.set("repository_discover", () => [
      {
        name: "nearby-fixture",
        path: "/tmp/nearby-fixture",
        gitdir: "/tmp/nearby-fixture/.git",
        workdir: "/tmp/nearby-fixture",
        isBare: false,
        isWorktree: false,
      } satisfies RepoRoot,
    ]);
    this.handlers.set("repository_remotes_load", () => ({
      remotes: [{
        name: "origin",
        fetchUrl: "https://example.invalid/flow/repository.git",
        pushUrl: null,
        fetchRefspecs: ["+refs/heads/*:refs/remotes/origin/*"],
        pushRefspecs: [],
        fetchMode: "default",
        trackedBranches: ["main"],
      }],
      upstreams: [],
    }));
    this.handlers.set("wc_status_load", () => this.status());
    this.handlers.set("wc_stage_paths", (payload) => {
      const paths = this.arrayArg(payload, "paths");
      this.entries = this.entries.map((item) => paths.includes(item.path) ? { ...item, staged: true } : item);
      return this.status();
    });
    this.handlers.set("wc_unstage_paths", (payload) => {
      const paths = this.arrayArg(payload, "paths");
      this.entries = this.entries.map((item) => paths.includes(item.path) ? { ...item, staged: false } : item);
      return this.status();
    });
    this.handlers.set("wc_paths_discard", (payload) => {
      const paths = this.arrayArg(payload, "paths");
      this.entries = this.entries.filter((item) => !paths.includes(item.path));
      return this.status();
    });
    this.handlers.set("wc_entry_assume_unchanged_set", (payload) => {
      const paths = this.arrayArg(payload, "paths");
      const value = this.commandArgs(payload).value === true;
      this.entries = this.entries.map((item) => paths.includes(item.path) ? { ...item, assumeUnchanged: value } : item);
      return this.status();
    });
    this.handlers.set("branch_list", (payload) => {
      const filter = this.commandArgs(payload).filter;
      if (filter === "remote") return [...this.remoteBranches];
      if (filter === "all") return [...this.localBranches, ...this.remoteBranches];
      return [...this.localBranches];
    });
    this.handlers.set("branch_create", (payload) => {
      const args = this.commandArgs(payload);
      const name = String(args.name ?? "feature/new");
      const created = branch(name, { isHead: args.checkout === true });
      this.localBranches = [...this.localBranches.map((item) => ({ ...item, isHead: args.checkout === true ? false : item.isHead })), created];
      if (args.checkout === true) this.repo = { ...this.repo, headBranch: name };
      return created;
    });
    this.handlers.set("branch_checkout", (payload) => {
      const name = String(this.commandArgs(payload).name ?? "main");
      this.localBranches = this.localBranches.map((item) => ({ ...item, isHead: item.name === name }));
      this.repo = { ...this.repo, headBranch: name };
      return { branch: name, oid: this.localBranches.find((item) => item.name === name)?.oid ?? OID_MAIN, detached: false };
    });
    this.handlers.set("branch_checkout_remote", (payload) => {
      const args = this.commandArgs(payload);
      const remoteBranch = String(args.remoteBranch ?? "origin/review").replace(/^refs\/remotes\//, "");
      const name = String(args.localName ?? remoteBranch.split("/").slice(1).join("/"));
      const created = branch(name, { isHead: true, upstream: remoteBranch, oid: OID_FEATURE });
      this.localBranches = [...this.localBranches.map((item) => ({ ...item, isHead: false })), created];
      this.repo = { ...this.repo, headBranch: name };
      return created;
    });
    this.handlers.set("branch_rename", (payload) => {
      const args = this.commandArgs(payload);
      const oldName = String(args.old ?? "feature/ready");
      const newName = String(args.new ?? "feature/renamed");
      this.localBranches = this.localBranches.map((item) => item.name === oldName ? branch(newName, { isHead: item.isHead, oid: item.oid }) : item);
      return this.localBranches.find((item) => item.name === newName) ?? branch(newName);
    });
    this.handlers.set("branch_delete", (payload) => {
      const names = this.arrayArg(payload, "names");
      this.localBranches = this.localBranches.filter((item) => !names.includes(item.name));
      return { deleted: names, skipped: [] };
    });
    this.handlers.set("history_commits_load", (payload) => {
      const args = this.commandArgs(payload);
      if (args.from) {
        const tail = this.commits[this.commits.length - 1] ?? commit(OID_FEATURE, "Implement a fixture feature", [OID_MAIN]);
        return { commits: [tail, commit(OID_OLDER, "Earlier fixture commit", [OID_FEATURE])], truncated: false };
      }
      return { commits: [...this.commits], truncated: true };
    });
    this.handlers.set("history_graph_compute", () => ({ lanes: this.commits.map(() => 0), segments: [], laneCount: 1 }));
    this.handlers.set("history_merge_base", () => ({ mergeBase: OID_MAIN, ahead: 1, behind: 1 }));
    this.handlers.set("commit_create", (payload) => this.createCommit(payload, false));
    this.handlers.set("commit_amend", (payload) => this.createCommit(payload, true));
    this.handlers.set("remote_fetch", () => ({ updatedRefs: ["refs/remotes/origin/main"] }));
    this.handlers.set("remote_pull", () => ({ commits: 1, mergeRequired: false }));
    this.handlers.set("remote_publish_branch", () => ({ updatedRefs: ["refs/heads/main"], rejected: [] }));
    this.handlers.set("merge_start", () => ({ outcome: { kind: "merged", commit: OID_FEATURE, parents: [OID_MAIN, OID_FEATURE] }, conflicts: [], notes: [] }));
    this.handlers.set("rebase_start", () => ({ outcome: { kind: "rebaseFinished", head: OID_MAIN, applied: 1, dropped: 0 }, conflicts: [], notes: [] }));
    this.handlers.set("stash_save", () => ({ index: 0, message: "Flow fixture stash", date: 1_760_000_000, author: "Flow Fixture" }));
    this.handlers.set("app_info_load", () => ({
      name: "Meridian",
      version: "0.1.0",
      commit: "fixture",
      buildProfile: "release",
      rustVersion: "1.90.0",
      osName: "macOS",
      osVersion: "15.0",
      arch: "aarch64",
      repoSlug: "example/meridian",
      updatesConfigured: true,
      canSelfUpdate: true,
      diagnostics: "Meridian 0.1.0 flow fixture",
    }));
    this.handlers.set("app_release_notes_load", (payload) => ({
      markdown: "## Flow fixture",
      locale: this.commandArgs(payload).locale === "en" ? "en" : "zh_cn",
      currentVersion: "0.1.0",
    }));
    this.handlers.set("app_update_check", () => ({
      kind: "checked",
      status: { currentVersion: "0.1.0", latestVersion: "0.1.0", notes: null, publishedAt: null, downloadBytes: null },
    }));
    this.handlers.set("app_feedback_compose", () => ({ url: "https://github.com/example/meridian/issues/new", prefilled: [] }));
    this.handlers.set("diagnostics_debug_get", () => ({ enabled: false, source: "off", logDir: "~/Library/Logs/dev.meridian.app", logFile: "~/Library/Logs/dev.meridian.app/meridian.log", level: "info" }));
    this.handlers.set("diagnostics_debug_set", (payload) => ({ enabled: this.commandArgs(payload).enabled === true, source: "setting", logDir: "~/Library/Logs/dev.meridian.app", logFile: "~/Library/Logs/dev.meridian.app/meridian.log", level: this.commandArgs(payload).enabled ? "debug" : "info" }));
    this.handlers.set("diagnostics_last_crash", () => null);
    this.handlers.set("diagnostics_log_tail", () => "[INFO] Flow fixture ready");
    this.handlers.set("diagnostics_log_write", () => undefined);
    this.handlers.set("diagnostics_reveal_logs", () => undefined);
    this.handlers.set("diagnostics_export", () => ({ path: "/tmp/meridian-diagnostics.txt", bytes: 128 }));
    this.handlers.set("diagnostics_open_devtools", () => undefined);
    this.handlers.set("diagnostics_dismiss_crash", () => undefined);
  }

  private async defaultResult(name: string, payload?: Record<string, unknown>): Promise<unknown> {
    if (name === "wc_diff_load") {
      const args = this.commandArgs(payload);
      return { path: args.path, perspective: args.perspective ?? "workingTreeToIndex", isBinary: false, hunks: [], additions: 0, deletions: 0, truncated: false };
    }
    if (name === "wc_chunk_apply" || name === "wc_chunk_discard") return this.status();
    if (name === "wc_image_data_load") return { path: "src/app.ts", side: "new", mime: "text/plain", size: 0, dataBase64: "" };
    if (name === "history_commit_load") return this.commitDetail();
    if (name === "history_changeset_load") return { oid: OID_MAIN, parent: null, entries: [], stats: { filesChanged: 0, insertions: 0, deletions: 0 } };
    if (name === "app_relaunch" || name === "wc_entry_open" || name === "merge_abort" || name === "integration_continue" || name === "integration_skip" || name === "merge_tool_invoke" || name === "diagnostics_log_write" || name === "diagnostics_reveal_logs" || name === "diagnostics_dismiss_crash") return undefined;
    if (name === "diagnostics_export") return { path: "/tmp/meridian-diagnostics.txt", bytes: 128 };
    if (name === "diagnostics_log_tail") return "[INFO] Flow fixture ready";
    if (name === "diagnostics_debug_get") return { enabled: false, source: "off", logDir: "~/Library/Logs/dev.meridian.app", logFile: "~/Library/Logs/dev.meridian.app/meridian.log", level: "info" };
    if (name === "diagnostics_debug_set") return { enabled: false, source: "off", logDir: "~/Library/Logs/dev.meridian.app", logFile: "~/Library/Logs/dev.meridian.app/meridian.log", level: "info" };
    if (name === "diagnostics_last_crash") return null;
    if (name === "diagnostics_open_devtools") return undefined;
    if (name === "app_pick_directory") return this.pickerResult;
    if (name === "repository_discover") return [];
    if (name === "branch_list") return [];
    if (name === "history_commits_load") return { commits: [], truncated: false };
    if (name === "history_graph_compute") return { lanes: [], segments: [], laneCount: 0 };
    if (name === "history_merge_base") return { mergeBase: null, ahead: 0, behind: 0 };
    return {};
  }

  private repositoryFixture(): RepoInfo {
    return {
      id: REPO_ID,
      gitdir: REPO_PATH + "/.git",
      workdir: REPO_PATH,
      isBare: false,
      isEmpty: false,
      headBranch: "main",
      headOid: OID_MAIN,
      state: "clean",
      discovered: false,
    };
  }

  private commandArgs(payload?: Record<string, unknown>): Record<string, unknown> {
    const args = payload?.args;
    return args && typeof args === "object" ? args as Record<string, unknown> : {};
  }

  private arrayArg(payload: Record<string, unknown> | undefined, key: string): string[] {
    const value = this.commandArgs(payload)[key];
    return Array.isArray(value) ? value.map(String) : [];
  }

  private status(): StatusDelta {
    const staged = this.entries.filter((item) => item.staged).length;
    const untracked = this.entries.filter((item) => item.code === "untracked").length;
    const unstaged = this.entries.filter((item) => !item.staged && item.code !== "untracked").length;
    return {
      repo: this.repo.id,
      entries: [...this.entries],
      summary: { staged, unstaged, untracked, conflicted: 0 },
    };
  }

  private createCommit(payload: Record<string, unknown> | undefined, amend: boolean): unknown {
    const args = this.commandArgs(payload);
    const message = typeof args.message === "string" ? args.message : "Fixture commit";
    const next = commit(OID_OLDER, message, [OID_MAIN]);
    this.commits = amend ? [next, ...this.commits.slice(1)] : [next, ...this.commits];
    this.entries = this.entries.map((item) => item.staged ? { ...item, staged: false } : item);
    return { ...next, parentCount: next.parents.length, detached: false, amend };
  }

  private commitDetail(): Record<string, unknown> {
    const selected = this.commits[0] ?? commit(OID_MAIN, "Add fixture main branch");
    return {
      ...selected,
      body: "",
      parentCount: selected.parents.length,
      tree: OID_MAIN,
      refs: [],
      signature: { state: "unsigned" },
      coauthors: [],
      trailers: [],
      stats: { filesChanged: 0, insertions: 0, deletions: 0 },
    };
  }
}

export const fixtureRepoId = REPO_ID;
export const fixtureRepoPath = REPO_PATH;
export const fixtureBranchName = "feature/ready";
export const fixtureRemoteBranch = "refs/remotes/origin/review";
export const fixtureFilePath = "src/app.ts";

/** One backend per test file; `reset()` returns it to the open-repository fixture. */
export const backend = new FakeBackend();
