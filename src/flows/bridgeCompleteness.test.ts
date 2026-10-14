/**
 * 桥接层全导出可见失败保护：新增命令也必须进入诊断失败网
 * 路由：所有经 src/bridges/commands.ts 调用的界面
 * API：枚举真实 bridge 导出，调用 fake backend 的拒绝路径
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The harness first: it installs the Tauri mock, and the bridge captures
// `invoke` when it is evaluated.
import { backend, fixtureBranchName, fixtureFilePath, fixtureRepoId, fixtureRemoteBranch, flushFlow, mountApp, toasts, unmountApp } from "./harness";
import * as commands from "../bridges/commands";
import { recentFailures } from "../diagnostics/recorder";

beforeEach(() => vi.useFakeTimers());
afterEach(async () => {
  await unmountApp();
  vi.useRealTimers();
});

const HUNK = {
  path: fixtureFilePath,
  perspective: "workingTreeToIndex" as const,
  index: 0,
  oldStart: 1,
  oldLines: 1,
  newStart: 1,
  newLines: 1,
  header: "@@ -1 +1 @@",
};
const HEAD = { kind: "head" as const };
const BRANCH = { kind: "branch" as const, value: fixtureBranchName };

const argumentsFor: Record<string, unknown[]> = {
  call: ["meta_probe", {}],
  diagnosticsLogTail: [20],
  diagnosticsLogWrite: ["error", "meta failure", "flow-meta"],
  diagnosticsDebugGet: [],
  diagnosticsDebugSet: [true],
  diagnosticsRevealLogs: [],
  diagnosticsExport: ["flow report"],
  diagnosticsOpenDevtools: [],
  diagnosticsLastCrash: [],
  diagnosticsDismissCrash: [],
  repositoryOpen: ["/tmp/meridian-flow"],
  repositoryRefresh: [fixtureRepoId],
  repositoryInit: ["/tmp/new-flow-repo", { createGitignore: true }],
  repositoryClone: ["https://example.invalid/team/demo.git", { destination: "/tmp/demo" }],
  repositoryCloneDestination: ["https://example.invalid/team/demo.git"],
  repositoryDiscover: ["/tmp"],
  wcStatusLoad: [fixtureRepoId],
  wcStagePaths: [fixtureRepoId, [fixtureFilePath]],
  wcUnstagePaths: [fixtureRepoId, [fixtureFilePath]],
  wcEntryAssumeUnchangedSet: [fixtureRepoId, [fixtureFilePath], true],
  historyCommitsLoad: [fixtureRepoId, { limit: 20 }],
  branchList: [fixtureRepoId, "all"],
  branchCreate: [fixtureRepoId, "feature/meta", HEAD, true],
  branchCheckout: [fixtureRepoId, fixtureBranchName],
  branchCheckoutRemote: [fixtureRepoId, fixtureRemoteBranch, "review", true],
  branchRename: [fixtureRepoId, fixtureBranchName, "feature/renamed"],
  branchDelete: [fixtureRepoId, [fixtureBranchName]],
  wcPathsDiscard: [fixtureRepoId, [fixtureFilePath]],
  wcEntryOpen: [fixtureRepoId, fixtureFilePath, "editor"],
  wcDiffLoad: [fixtureRepoId, fixtureFilePath, "workingTreeToIndex"],
  wcChunkApply: [fixtureRepoId, HUNK, "stage"],
  wcChunkDiscard: [fixtureRepoId, HUNK],
  wcImageDataLoad: [fixtureRepoId, "image.png", "new"],
  historyCommitLoad: [fixtureRepoId, "a123456"],
  historyChangesetLoad: [fixtureRepoId, "a123456"],
  historyCompareSummary: [fixtureRepoId, HEAD, BRANCH],
  historyGraphCompute: [fixtureRepoId, ["a123456"]],
  commitCreate: [fixtureRepoId, "meta commit"],
  commitAmend: [fixtureRepoId, "meta amend"],
  repositoryRemotesLoad: [fixtureRepoId],
  remoteFetch: [fixtureRepoId, "origin"],
  remotePull: [fixtureRepoId, "origin"],
  remotePublishBranch: [fixtureRepoId, "origin", "main"],
  mergeStart: [fixtureRepoId, BRANCH, { autoCommit: true }],
  mergeAbort: [fixtureRepoId],
  rebaseStart: [fixtureRepoId, BRANCH],
  integrationContinue: [fixtureRepoId],
  integrationSkip: [fixtureRepoId],
  mergeToolInvoke: [fixtureRepoId, [fixtureFilePath]],
  wcMergeStatusLoad: [fixtureRepoId],
  stashSave: [fixtureRepoId],
  historyContributorsLoad: [fixtureRepoId],
  appInfoLoad: [],
  appReleaseNotesLoad: ["zh-CN"],
  appUpdateCheck: [],
  appUpdateInstall: [() => undefined],
  appFeedbackCompose: [{ kind: "bug", summary: "flow test" }],
  relaunch: [],
  pickDirectory: [],
};

/**
 * Call one exported wrapper and return the wire command it reached.
 *
 * The wire name is read from the fake backend rather than guessed from the
 * export: an export name and its command name differ in case and vocabulary
 * (`repositoryOpen` → `repository_open`), and `call` is the generic primitive
 * every other wrapper delegates to, so only the backend knows what ran.
 */
async function invokeExport(name: string, implementation: unknown): Promise<string> {
  const args = argumentsFor[name];
  expect(args, `Missing test arguments for bridge export ${name}`).toBeDefined();
  const before = backend.calls.length;
  await expect((implementation as (...values: unknown[]) => Promise<unknown>)(...args!)).rejects.toBeDefined();
  const reached = backend.calls.slice(before).at(-1)?.name;
  expect(reached, `${name} should reach invoke`).toBeDefined();
  return reached!;
}

describe("every command bridge rejection is visible", () => {
  it("enumerates every runtime export that reaches invoke and verifies recorder plus danger toast", async () => {
    await mountApp();
    backend.rejectAll();
    const exports = Object.entries(commands).filter(([, value]) => typeof value === "function");
    expect(exports.map(([name]) => name).sort()).toEqual(Object.keys(argumentsFor).sort());

    for (const [name, implementation] of exports) {
      const previousToasts = toasts("danger").length;
      const command = await invokeExport(name, implementation);
      await flushFlow(300);
      expect(
        [...recentFailures()].reverse().find((record) => record.command === command),
        `${name} should be retained in diagnostics`,
      ).toBeDefined();
      expect(toasts("danger").length, `${name} should show a danger toast`).toBeGreaterThan(previousToasts);
      // The net de-duplicates by message inside two seconds and caps at five per
      // ten; moving past both windows keeps every command's toast countable.
      await vi.advanceTimersByTimeAsync(10_001);
    }
  });
});
