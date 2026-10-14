/**
 * The TypeScript wrappers are executable evidence of what the webview sends.
 * Keep one sample per exported command and pin the captured IPC payloads beside
 * the Rust crate, where its source scanner checks the other half of the wire.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class Channel<T> {
    onmessage: ((value: T) => void) | null = null;
  },
}));

import { Channel, invoke } from "@tauri-apps/api/core";
import * as commands from "./commands";
import type { FeedbackRequest, HunkRef, Revision } from "./bindings";

const invokeMock = vi.mocked(invoke);
const fixturePath = resolve(process.cwd(), "src-tauri/tests/fixtures/ipc-contract.json");
const rustLibPath = resolve(process.cwd(), "src-tauri/src/lib.rs");
const repo = "repo-contract";
const head: Revision = { kind: "head" };
const branch: Revision = { kind: "branch", value: "main" };
const committer = { name: "Contract Tester", email: "test@example.com", time: 1_700_000_000 };
const hunk: HunkRef = {
  path: "README.md",
  perspective: "workingTreeToIndex",
  index: 0,
  oldStart: 1,
  oldLines: 1,
  newStart: 1,
  newLines: 1,
  header: "@@ -1 +1 @@",
};

/** Add a wrapper here whenever commands.ts exports a new command. */
const SAMPLES: Record<string, () => Promise<unknown>> = {
  // The exported instrumentation primitive also crosses IPC. Use a registered
  // no-argument command; callers with bespoke payloads need their own wrapper.
  call: () => commands.call("app_info_load"),
  diagnosticsLogTail: () => commands.diagnosticsLogTail(20),
  diagnosticsLogWrite: () => commands.diagnosticsLogWrite("error", "sample failure", "ipc-contract"),
  diagnosticsDebugGet: () => commands.diagnosticsDebugGet(),
  diagnosticsDebugSet: () => commands.diagnosticsDebugSet(true),
  diagnosticsRevealLogs: () => commands.diagnosticsRevealLogs(),
  diagnosticsExport: () => commands.diagnosticsExport("contract sample"),
  diagnosticsOpenDevtools: () => commands.diagnosticsOpenDevtools(),
  diagnosticsLastCrash: () => commands.diagnosticsLastCrash(),
  diagnosticsDismissCrash: () => commands.diagnosticsDismissCrash(),
  repositoryOpen: () => commands.repositoryOpen("/tmp/meridian", "dialogue"),
  repositoryRefresh: () => commands.repositoryRefresh(repo),
  repositoryInit: () => commands.repositoryInit("/tmp/meridian-init", {
    bare: false,
    initialBranch: "main",
    createGitignore: true,
  }),
  repositoryClone: () => commands.repositoryClone("https://example.com/meridian.git", {
    destination: "/tmp/meridian-clone",
    bare: false,
    initialBranch: "main",
  }),
  repositoryCloneDestination: () => commands.repositoryCloneDestination("https://example.com/meridian.git"),
  repositoryDiscover: () => commands.repositoryDiscover("/tmp", { maxDepth: 2, ignored: [".git"] }),
  wcStatusLoad: () => commands.wcStatusLoad(repo, { includeIgnored: true, includeDirs: false }),
  wcStagePaths: () => commands.wcStagePaths(repo, ["README.md"]),
  wcUnstagePaths: () => commands.wcUnstagePaths(repo, ["README.md"]),
  wcEntryAssumeUnchangedSet: () => commands.wcEntryAssumeUnchangedSet(repo, ["README.md"], true),
  historyCommitsLoad: () => commands.historyCommitsLoad(repo, {
    from: head,
    to: branch,
    path: "README.md",
    skip: 0,
    limit: 25,
    sort: "date",
    firstParent: false,
    followRenames: true,
    search: "sample",
    searchMode: "message",
  }),
  branchList: () => commands.branchList(repo, "all", "main"),
  branchCreate: () => commands.branchCreate(repo, "contract", branch, true, "dialogue"),
  branchCheckout: () => commands.branchCheckout(repo, "main", { detach: false }),
  branchCheckoutRemote: () => commands.branchCheckoutRemote(repo, "origin/main", "main", true),
  branchRename: () => commands.branchRename(repo, "old", "new", false),
  branchDelete: () => commands.branchDelete(repo, ["old"], false, "dialogue"),
  wcPathsDiscard: () => commands.wcPathsDiscard(repo, ["README.md"], {
    includeUntracked: false,
    confirmToken: "confirmed",
  }),
  wcEntryOpen: () => commands.wcEntryOpen(repo, "README.md", "editor"),
  wcDiffLoad: () => commands.wcDiffLoad(repo, "README.md", "workingTreeToIndex", {
    contextLines: 3,
    ignoreWhitespace: false,
    showFull: false,
    syntax: "markdown",
  }),
  wcChunkApply: () => commands.wcChunkApply(repo, hunk, "stage", 3),
  wcChunkDiscard: () => commands.wcChunkDiscard(repo, hunk, 3),
  wcImageDataLoad: () => commands.wcImageDataLoad(repo, "image.png", "new"),
  historyCommitLoad: () => commands.historyCommitLoad(repo, "0123456789abcdef0123456789abcdef01234567"),
  historyChangesetLoad: () => commands.historyChangesetLoad(repo, "0123456789abcdef0123456789abcdef01234567", true),
  historyCompareSummary: () => commands.historyCompareSummary(repo, head, branch),
  historyGraphCompute: () => commands.historyGraphCompute(repo, ["0123456789abcdef0123456789abcdef01234567"]),
  commitCreate: () => commands.commitCreate(repo, "sample commit", {
    signOff: true,
    sign: { kind: "none" },
    skipHooks: false,
    templateUsed: false,
    committer,
  }),
  commitAmend: () => commands.commitAmend(repo, null, {
    sign: { kind: "gpg", key: "test-key" },
    signOff: true,
    skipHooks: false,
    templateUsed: false,
    committer,
  }),
  repositoryRemotesLoad: () => commands.repositoryRemotesLoad(repo),
  remoteFetch: () =>
    commands.remoteFetch(repo, "origin", ["+refs/heads/*:refs/remotes/origin/*"], {
      prune: true,
      tags: "follow",
    }),
  remotePull: () => commands.remotePull(repo, "origin", null, { mode: "merge", message: "" }),
  remotePublishBranch: () => commands.remotePublishBranch(repo, "origin", "main", true),
  mergeStart: () => commands.mergeStart(repo, branch, {
    alwaysMergeCommit: false,
    autoCommit: true,
    verifySignatures: false,
    autostash: false,
    squash: false,
    strategy: "default",
    message: "",
  }),
  mergeAbort: () => commands.mergeAbort(repo),
  rebaseStart: () => commands.rebaseStart(repo, branch, null, { autostash: false, verifySignatures: false }),
  integrationContinue: () => commands.integrationContinue(repo, null),
  integrationSkip: () => commands.integrationSkip(repo),
  mergeToolInvoke: () => commands.mergeToolInvoke(repo, ["README.md"]),
  wcMergeStatusLoad: () => commands.wcMergeStatusLoad(repo, null),
  stashSave: () => commands.stashSave(repo, {
    message: "sample",
    includeUntracked: false,
    includeIgnored: false,
    keepIndex: false,
    paths: [],
  }),
  historyContributorsLoad: () => commands.historyContributorsLoad(repo),
  appInfoLoad: () => commands.appInfoLoad(),
  appReleaseNotesLoad: () => commands.appReleaseNotesLoad("en"),
  appUpdateCheck: () => commands.appUpdateCheck(),
  appUpdateInstall: () => commands.appUpdateInstall(() => undefined),
  appFeedbackCompose: () => commands.appFeedbackCompose({
    kind: "bug",
    summary: "sample report",
    context: "sample diagnostics",
  } satisfies FeedbackRequest),
  relaunch: () => commands.relaunch(),
  pickDirectory: () => commands.pickDirectory(),
};

// Exercise defaults as well as populated option objects: `opts: {}` must be
// deserializable, and a valid enum's alternate variants must remain valid too.
const EXTRA_SAMPLES: Record<string, Record<string, () => Promise<unknown>>> = {
  repositoryOpen: { defaults: () => commands.repositoryOpen("/tmp/meridian") },
  repositoryInit: { defaults: () => commands.repositoryInit("/tmp/meridian-init") },
  repositoryClone: { defaults: () => commands.repositoryClone("https://example.com/meridian.git") },
  repositoryDiscover: { defaults: () => commands.repositoryDiscover("/tmp") },
  wcStatusLoad: { defaults: () => commands.wcStatusLoad(repo) },
  historyCommitsLoad: { defaults: () => commands.historyCommitsLoad(repo) },
  branchList: { defaults: () => commands.branchList(repo) },
  branchCreate: { defaults: () => commands.branchCreate(repo, "contract") },
  branchCheckout: { defaults: () => commands.branchCheckout(repo, "main") },
  branchCheckoutRemote: { defaults: () => commands.branchCheckoutRemote(repo, "origin/main") },
  branchRename: { defaults: () => commands.branchRename(repo, "old", "new") },
  branchDelete: { defaults: () => commands.branchDelete(repo, ["old"]) },
  wcPathsDiscard: { defaults: () => commands.wcPathsDiscard(repo, ["README.md"]) },
  wcDiffLoad: { defaults: () => commands.wcDiffLoad(repo, null, "indexToBase") },
  wcChunkApply: { defaults: () => commands.wcChunkApply(repo, hunk, "unstage") },
  wcChunkDiscard: { defaults: () => commands.wcChunkDiscard(repo, hunk) },
  historyChangesetLoad: { defaults: () => commands.historyChangesetLoad(repo, "0123456789abcdef0123456789abcdef01234567") },
  commitCreate: { defaults: () => commands.commitCreate(repo, "sample commit") },
  commitAmend: { defaults: () => commands.commitAmend(repo, null) },
  remoteFetch: { defaults: () => commands.remoteFetch(repo, "origin") },
  remotePull: {
    defaults: () => commands.remotePull(repo, "origin"),
    fastForwardOnly: () => commands.remotePull(repo, "origin", "main", { mode: "fastForwardOnly" }),
    rebase: () => commands.remotePull(repo, "origin", "main", { mode: "rebase" }),
  },
  remotePublishBranch: { defaults: () => commands.remotePublishBranch(repo, "origin", "main") },
  mergeStart: { defaults: () => commands.mergeStart(repo, branch) },
  rebaseStart: { defaults: () => commands.rebaseStart(repo, branch) },
  integrationContinue: { defaults: () => commands.integrationContinue(repo) },
  wcMergeStatusLoad: { defaults: () => commands.wcMergeStatusLoad(repo) },
  stashSave: { defaults: () => commands.stashSave(repo) },
  appReleaseNotesLoad: { defaults: () => commands.appReleaseNotesLoad() },
  appFeedbackCompose: { minimal: () => commands.appFeedbackCompose({ kind: "feature" }) },
  diagnosticsLogWrite: { withoutSource: () => commands.diagnosticsLogWrite("info", "sample") },
  diagnosticsExport: { withoutContext: () => commands.diagnosticsExport() },
};

type CapturedCall = { wrapper: string; case: string; command: string; payload: unknown };

function stable(value: unknown): unknown {
  if (value instanceof Channel) return { $channel: true };
  if (Array.isArray(value)) return value.map((item) => stable(item));
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, item]) => item !== undefined)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, item]) => [key, stable(item)]),
    );
  }
  return value;
}

function registeredCommands(): Set<string> {
  const source = readFileSync(rustLibPath, "utf8").replace(/\/\*[\s\S]*?\*\/|\/\/[^\n]*/g, "");
  const body = source.split("generate_handler![")[1]?.split("])", 1)[0];
  expect(body, "src-tauri/src/lib.rs contains generate_handler!").toBeTruthy();
  return new Set(Array.from(body?.matchAll(/commands::([A-Za-z0-9_]+)/g) ?? [], (match) => match[1]));
}

describe("TypeScript ↔ Rust IPC payload contract", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("captures a sample for every command wrapper and checks handler registration", async () => {
    const exportedFunctions = Object.entries(commands)
      .filter(([, value]) => typeof value === "function")
      .map(([name]) => name)
      .sort();
    const sampled = Object.keys(SAMPLES).sort();
    const unclassified = exportedFunctions.filter((name) => !Object.hasOwn(SAMPLES, name));
    const staleSamples = sampled.filter((name) => !exportedFunctions.includes(name));

    expect(unclassified, "add an IPC sample for each new commands.ts function export").toEqual([]);
    expect(staleSamples, "remove samples when a bridge wrapper is removed").toEqual([]);
    expect(Object.keys(EXTRA_SAMPLES).filter((name) => !Object.hasOwn(SAMPLES, name))).toEqual([]);

    const actual: CapturedCall[] = [];
    invokeMock.mockResolvedValue(undefined as never);
    for (const wrapper of sampled) {
      const cases = { populated: SAMPLES[wrapper]!, ...EXTRA_SAMPLES[wrapper] };
      for (const [caseName, sample] of Object.entries(cases).sort(([a], [b]) => a.localeCompare(b))) {
        invokeMock.mockClear();
        await sample();
        expect(invokeMock, `${wrapper} [${caseName}] must invoke exactly once`).toHaveBeenCalledTimes(1);
        const [command, payload] = invokeMock.mock.calls[0]!;
        actual.push({ wrapper, case: caseName, command, payload: stable(payload ?? null) });
      }
    }
    actual.sort((left, right) => `${left.command}/${left.wrapper}/${left.case}`.localeCompare(`${right.command}/${right.wrapper}/${right.case}`));

    const registered = registeredCommands();
    const missing = actual.filter(({ command }) => !registered.has(command));
    expect(missing, "every captured IPC command is registered by generate_handler!").toEqual([]);

    if (process.env.UPDATE_IPC_CONTRACT === "1") {
      writeFileSync(fixturePath, `${JSON.stringify(actual, null, 2)}\n`);
    }
    const fixture = JSON.parse(readFileSync(fixturePath, "utf8")) as CapturedCall[];
    expect(actual, "wire payload drift; review and update with UPDATE_IPC_CONTRACT=1").toEqual(fixture);
  });
});
