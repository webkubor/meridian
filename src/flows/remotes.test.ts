/**
 * 抓取、拉取、推送与储藏的用户可见流程回归
 * 路由：仓库工具栏与分支详情页
 * API：远程和集成命令通过真实 Vue composables、stores 与命令桥接层
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";

import {
  backend,
  clickToolbarVerb,
  clickWorkspaceTab,
  fixtureBranchName,
  fixtureRepoId,
  flushFlow,
  hasDangerToast,
  hasSuccessToast,
  mountApp,
  openRepositoryFixture,
  recordedFailures,
  unmountApp,
} from "./harness";

beforeEach(() => vi.useFakeTimers());
afterEach(async () => {
  await unmountApp();
  vi.useRealTimers();
});

async function repositoryScreen(): Promise<VueWrapper> {
  const wrapper = await mountApp();
  await openRepositoryFixture(wrapper);
  return wrapper;
}

/** Push the branch the pane is showing, not the checked-out one. */
async function pushFromBranchPane(wrapper: VueWrapper): Promise<void> {
  await clickWorkspaceTab("分支", wrapper);
  const row = wrapper.findAll(".branch-row").find((candidate) => candidate.text().includes(fixtureBranchName));
  await row!.trigger("click");
  await flushFlow();
  const push = wrapper.findAll(".branch-detail button").find((button) => button.text().trim() === "推送");
  if (!push) throw new Error("The branch pane has no push control");
  await push.trigger("click");
  await flushFlow();
}

/**
 * A repo-level git verb reports through `runVerb`: one shared fallback sentence
 * plus the machine code on screen, the engine's own message in diagnostics.
 */
function expectVerbFailure(command: string, engineMessage: string): void {
  expect(hasDangerToast("操作失败。")).toBe(true);
  expect(recordedFailures().find((failure) => failure.command === command)?.error.message).toBe(engineMessage);
}

describe("remote routine operations", () => {
  it("fetches the default remote from the toolbar", async () => {
    const wrapper = await repositoryScreen();
    await clickToolbarVerb("抓取", wrapper);

    expect(backend.call("remote_fetch")?.payload).toEqual({ args: { repo: fixtureRepoId, remote: "origin", refspecs: [], mode: {} } });
    expect(hasSuccessToast("抓取完成。")).toBe(true);
  });

  it("surfaces a fetch network failure and records it", async () => {
    const wrapper = await repositoryScreen();
    backend.failNext("remote_fetch", {
      code: "io",
      message: "无法连接 origin。",
      detail: "connection timed out",
      params: { remote: "origin" },
      retryable: true,
      domain: "git",
    });
    await clickToolbarVerb("抓取", wrapper);
    await flushFlow(300);

    expectVerbFailure("remote_fetch", "无法连接 origin。");
  });

  it("pulls the default remote and refreshes repository data", async () => {
    const wrapper = await repositoryScreen();
    const before = backend.calls.filter((call) => ["branch_list", "wc_status_load", "history_commits_load"].includes(call.name)).length;
    await clickToolbarVerb("拉取", wrapper);

    expect(backend.call("remote_pull")?.payload).toEqual({ args: { repo: fixtureRepoId, remote: "origin", branch: null, opts: {} } });
    expect(hasSuccessToast("拉取完成。")).toBe(true);
    expect(backend.calls.filter((call) => ["branch_list", "wc_status_load", "history_commits_load"].includes(call.name)).length).toBeGreaterThan(before);
  });

  it("shows a pull failure instead of silently leaving stale state", async () => {
    const wrapper = await repositoryScreen();
    backend.failNext("remote_pull", {
      code: "checkout_conflicts",
      message: "拉取会覆盖本地改动。",
      detail: "local changes would be overwritten",
      params: {},
      retryable: false,
      domain: "git",
    });
    await clickToolbarVerb("拉取", wrapper);
    await flushFlow(300);

    expectVerbFailure("remote_pull", "拉取会覆盖本地改动。");
  });

  it("pushes the current branch from the toolbar", async () => {
    const wrapper = await repositoryScreen();
    await clickToolbarVerb("推送", wrapper);

    expect(backend.call("remote_publish_branch")?.payload).toEqual({ args: { repo: fixtureRepoId, remote: "origin", branch: "main", setUpstream: false } });
    expect(hasSuccessToast("已推送")).toBe(true);
  });

  /**
   * BUG: the toolbar's push toast interpolates an empty branch name.
   *
   * `App.vue`'s `onToolbarRun` calls `runVerb(() => git.push(), t("shell.toast.pushed"))`
   * with no `{ branch }` interpolation, so the sentence renders as 已推送 。 while
   * the branch pane's own push (which does pass the name) reads correctly.
   * Expected fix in `src/App.vue`: pass the branch, as `pushSelectedBranch` does.
   */
  it.fails("BUG: the toolbar push toast names the branch it published", async () => {
    const wrapper = await repositoryScreen();
    await clickToolbarVerb("推送", wrapper);

    expect(hasSuccessToast("已推送 main。")).toBe(true);
  });

  it("shows a toolbar push rejection with a visible error and a diagnostic record", async () => {
    const wrapper = await repositoryScreen();
    backend.failNext("remote_publish_branch", {
      code: "invalid_request",
      message: "远程拒绝了这次推送。",
      detail: "non-fast-forward update rejected",
      params: { branch: "main" },
      retryable: false,
      domain: "git",
    });
    await clickToolbarVerb("推送", wrapper);
    await flushFlow(300);

    expectVerbFailure("remote_publish_branch", "远程拒绝了这次推送。");
  });

  it("pushes the branch shown in the branch pane rather than the checked-out branch", async () => {
    const wrapper = await repositoryScreen();
    await pushFromBranchPane(wrapper);

    expect(backend.call("remote_publish_branch")?.payload).toEqual({ args: { repo: fixtureRepoId, remote: "origin", branch: fixtureBranchName, setUpstream: false } });
    expect(hasSuccessToast(`已推送 ${fixtureBranchName}。`)).toBe(true);
  });

  it("shows failure from the branch-pane push action", async () => {
    const wrapper = await repositoryScreen();
    backend.failNext("remote_publish_branch", {
      code: "invalid_request",
      message: "分支推送被远程拒绝。",
      detail: "protected branch hook declined update",
      params: { branch: fixtureBranchName },
      retryable: false,
      domain: "git",
    });
    await pushFromBranchPane(wrapper);
    await flushFlow(300);

    expectVerbFailure("remote_publish_branch", "分支推送被远程拒绝。");
  });

  it("stashes the working copy and reports completion", async () => {
    const wrapper = await repositoryScreen();
    await clickToolbarVerb("储藏", wrapper);

    expect(backend.call("stash_save")?.payload).toEqual({ args: { repo: fixtureRepoId, opts: {} } });
    expect(hasSuccessToast("已储藏工作区。")).toBe(true);
  });

  it("shows stash failure and records the rejected command", async () => {
    const wrapper = await repositoryScreen();
    backend.failNext("stash_save", {
      code: "invalid_request",
      message: "工作区没有可储藏的改动。",
      detail: "stash_save called on a clean repository",
      params: {},
      retryable: false,
      domain: "git",
    });
    await clickToolbarVerb("储藏", wrapper);
    await flushFlow(300);

    expectVerbFailure("stash_save", "工作区没有可储藏的改动。");
  });
});
