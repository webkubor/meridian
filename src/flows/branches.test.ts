/**
 * 分支创建、签出、改名、删除、合并与变基的真实界面流程
 * 路由：/repo/:repoId 的分支页签
 * API：真实 Vue UI 和 Pinia stores 经命令桥接层调用 fake Tauri backend
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";

import {
  backend,
  checkDialogueOption,
  clickButton,
  clickWorkspaceTab,
  confirmDialogue,
  fixtureBranchName,
  fixtureRepoId,
  flushFlow,
  hasDangerToast,
  hasSuccessToast,
  mountApp,
  openRepositoryFixture,
  pickBranchInDialogue,
  recordedFailures,
  typeInDialogue,
  unmountApp,
} from "./harness";

beforeEach(() => vi.useFakeTimers());
afterEach(async () => {
  await unmountApp();
  vi.useRealTimers();
});

async function branchScreen(): Promise<VueWrapper> {
  const wrapper = await mountApp();
  await openRepositoryFixture(wrapper);
  await clickWorkspaceTab("分支", wrapper);
  return wrapper;
}

async function selectBranch(wrapper: VueWrapper, name: string): Promise<void> {
  const row = wrapper.findAll(".branch-row").find((candidate) => candidate.text().includes(name));
  if (!row) throw new Error(`Branch ${JSON.stringify(name)} is not visible`);
  await row.trigger("click");
  await flushFlow();
}

async function detailVerb(wrapper: VueWrapper, label: string | RegExp): Promise<void> {
  const button = wrapper.findAll(".branch-detail button").find((candidate) =>
    typeof label === "string" ? candidate.text().trim() === label : label.test(candidate.text()),
  );
  if (!button) throw new Error(`The branch pane has no control matching ${String(label)}`);
  await button.trigger("click");
  await flushFlow();
}

/** Accept the dirty-working-tree confirmation when the shell asks for one. */
async function confirmDirtyIfAsked(): Promise<void> {
  const panel = document.querySelector('[role="dialog"]');
  if (!(panel instanceof HTMLElement)) return;
  expect(panel.textContent).toContain("未提交的改动");
  await confirmDialogue();
}

/** Switch the branch list scope; the sidebar has a section with the same name. */
async function setBranchFilter(wrapper: VueWrapper, label: string): Promise<void> {
  const tab = wrapper.findAll("button.filter-tab").find((candidate) => candidate.text().trim() === label);
  if (!tab) throw new Error(`The branch list has no ${JSON.stringify(label)} filter`);
  await tab.trigger("click");
  await flushFlow();
}

/** Open the toolbar's branch split-action and choose one of its options. */
async function chooseToolbarBranchOption(wrapper: VueWrapper, optionText: string): Promise<void> {
  const anchor = wrapper.findAll(".popover-anchor").find((candidate) => candidate.text().includes("分支"));
  await anchor!.get("button.split-caret").trigger("click");
  await flushFlow();
  const option = wrapper.findAll('[role="menuitem"]').find((item) => item.text().includes(optionText));
  if (!option) throw new Error(`The branch menu has no ${JSON.stringify(optionText)} option`);
  await option.trigger("click");
  await flushFlow();
}

/**
 * A repo-level git verb reports through `runVerb`: the catalogue sentence for
 * the failure's code, with the engine's own message under it, and the same
 * message in the diagnostics recorder. One shared "the action failed" sentence
 * used to be the whole of it, which is exactly what the owner complained about.
 */
function expectVerbFailure(command: string, engineMessage: string): void {
  expect(hasDangerToast(engineMessage)).toBe(true);
  expect(recordedFailures().find((failure) => failure.command === command)?.error.message).toBe(engineMessage);
}

describe("branch operations", () => {
  it("creates a branch with the checkout checkbox enabled", async () => {
    const wrapper = await branchScreen();
    await clickButton("新建分支", wrapper);
    await typeInDialogue("feature/flow-checked");
    await confirmDialogue();

    expect(backend.call("branch_create")?.payload).toEqual({ args: {
      repo: fixtureRepoId,
      name: "feature/flow-checked",
      start: { kind: "head" },
      checkout: true,
      sender: "dialogue",
    } });
    expect(hasSuccessToast("已创建分支 feature/flow-checked。")).toBe(true);
  });

  it("creates a branch without checking it out when the checkbox is off", async () => {
    const wrapper = await branchScreen();
    await clickButton("新建分支", wrapper);
    await checkDialogueOption(false);
    await typeInDialogue("feature/flow-no-checkout");
    await confirmDialogue();

    expect(backend.call("branch_create")?.payload).toEqual({ args: {
      repo: fixtureRepoId,
      name: "feature/flow-no-checkout",
      start: { kind: "head" },
      checkout: false,
      sender: "dialogue",
    } });
    expect(hasSuccessToast("已创建分支 feature/flow-no-checkout。")).toBe(true);
  });

  it("surfaces create rejection in a danger toast and records the failure", async () => {
    const wrapper = await branchScreen();
    backend.failNext("branch_create", {
      code: "reference_exists",
      message: "此分支名已被使用。",
      detail: "refs/heads/feature/flow-checked already exists",
      params: { name: "feature/flow-checked" },
      retryable: false,
      domain: "git",
    });
    await clickButton("新建分支", wrapper);
    await typeInDialogue("feature/flow-checked");
    await confirmDialogue();
    await flushFlow(300);

    // The catalogue sentence for the code, with the engine's own words under
    // it: "无法创建该分支。" said nothing about which failure it was.
    expect(hasDangerToast("「{name}」已存在。".replace("{name}", "feature/flow-checked"))).toBe(true);
    expect(hasDangerToast("reference_exists")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "branch_create")).toBe(true);
  });

  it("checks out a selected local branch with the exact payload", async () => {
    const wrapper = await branchScreen();
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, "签出");
    await confirmDirtyIfAsked();

    expect(backend.call("branch_checkout")?.payload).toEqual({ args: { repo: fixtureRepoId, name: fixtureBranchName, opts: {} } });
    expect(hasSuccessToast("已签出")).toBe(true);
  });

  it("confirms a local checkout failure and keeps the engine message in diagnostics", async () => {
    const wrapper = await branchScreen();
    backend.failNext("branch_checkout", {
      code: "checkout_conflicts",
      message: "签出会覆盖本地改动。",
      detail: "checkout would overwrite a modified file",
      params: { branch: fixtureBranchName },
      retryable: false,
      domain: "git",
    });
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, "签出");
    await confirmDirtyIfAsked();
    await flushFlow(300);

    expect(hasDangerToast("签出会覆盖本地改动。")).toBe(true);
    expect(recordedFailures().find((failure) => failure.command === "branch_checkout")?.error.message).toBe("签出会覆盖本地改动。");
  });

  /**
   * A row that looks selectable and selects nothing.
   *
   * `BranchListPane` lists local *and* remote branches and emits the selected
   * row's `fullName`, so the detail pane has to resolve the selection against
   * that same set; resolving it against the local-only list silently leaves the
   * remote row's verbs — 抓取到本地 among them — unreachable.
   */
  it("a selected remote branch offers its own verbs in the detail pane", async () => {
    const wrapper = await branchScreen();
    await setBranchFilter(wrapper, "远程");
    await selectBranch(wrapper, "review");

    expect(wrapper.find(".branch-detail").text()).toContain("review");
    await detailVerb(wrapper, "抓取到本地");
    await confirmDirtyIfAsked();
    expect(backend.call("branch_checkout_remote")?.payload).toEqual({ args: {
      repo: fixtureRepoId,
      remoteBranch: "origin/review",
      localName: "review",
      track: true,
    } });
  });

  it("checks out a remote branch through the toolbar dialogue, with the dirty-tree confirm", async () => {
    const wrapper = await branchScreen();
    await chooseToolbarBranchOption(wrapper, "签出分支");
    await pickBranchInDialogue("review");
    await confirmDialogue();
    await confirmDirtyIfAsked();

    expect(backend.call("branch_checkout_remote")?.payload).toEqual({ args: {
      repo: fixtureRepoId,
      remoteBranch: "origin/review",
      localName: "review",
      track: true,
    } });
    expect(hasSuccessToast("已签出")).toBe(true);
  });

  it("shows remote checkout failure after the confirmation is accepted", async () => {
    const wrapper = await branchScreen();
    backend.failNext("branch_checkout_remote", {
      code: "reference_not_found",
      message: "远程分支已不存在。",
      detail: "origin/review is missing",
      params: { branch: "origin/review" },
      retryable: false,
      domain: "git",
    });
    await chooseToolbarBranchOption(wrapper, "签出分支");
    await pickBranchInDialogue("review");
    await confirmDialogue();
    await confirmDirtyIfAsked();
    await flushFlow(300);

    expect(hasDangerToast("远程分支已不存在。")).toBe(true);
    expect(hasDangerToast("reference_not_found")).toBe(true);
    expect(recordedFailures().find((failure) => failure.command === "branch_checkout_remote")?.error.message).toBe("远程分支已不存在。");
  });

  it("renames a selected branch and shows the updated name", async () => {
    const wrapper = await branchScreen();
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, "重命名");
    await typeInDialogue("feature/flow-renamed");
    await confirmDialogue();

    expect(backend.call("branch_rename")?.payload).toEqual({ args: { repo: fixtureRepoId, old: fixtureBranchName, new: "feature/flow-renamed", force: false } });
    expect(hasSuccessToast("已重命名为 feature/flow-renamed。")).toBe(true);
  });

  it("shows branch rename failure visibly and in diagnostics", async () => {
    const wrapper = await branchScreen();
    backend.failNext("branch_rename", {
      code: "invalid_reference_name",
      message: "这个分支名无效。",
      detail: "invalid refname",
      params: { name: "feature/flow-renamed" },
      retryable: false,
      domain: "git",
    });
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, "重命名");
    await typeInDialogue("feature/flow-renamed");
    await confirmDialogue();
    await flushFlow(300);

    expect(hasDangerToast("「{name}」不是合法的分支名或标签名。".replace("{name}", "feature/flow-renamed"))).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "branch_rename")).toBe(true);
  });

  it("deletes only after confirming the named branch", async () => {
    const wrapper = await branchScreen();
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, "删除");
    expect(backend.call("branch_delete")).toBeUndefined();
    await confirmDialogue();

    expect(backend.call("branch_delete")?.payload).toEqual({ args: { repo: fixtureRepoId, names: [fixtureBranchName], force: false, sender: "dialogue" } });
    expect(hasSuccessToast(`已删除 ${fixtureBranchName}。`)).toBe(true);
  });

  it("shows a branch delete failure after confirmation", async () => {
    const wrapper = await branchScreen();
    backend.failNext("branch_delete", {
      code: "invalid_request",
      message: "未合并的分支不能删除。",
      detail: "branch contains commits not merged into HEAD",
      params: { branch: fixtureBranchName },
      retryable: false,
      domain: "git",
    });
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, "删除");
    await confirmDialogue();
    await flushFlow(300);

    expect(hasDangerToast("Meridian 拒绝了这个请求。")).toBe(true);
    expect(hasDangerToast("未合并的分支不能删除。")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "branch_delete")).toBe(true);
  });

  it("merges a selected branch with the auto-commit option the shell sends", async () => {
    const wrapper = await branchScreen();
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, /^合并到/);

    expect(backend.call("merge_start")?.payload).toEqual({ args: {
      repo: fixtureRepoId,
      revision: { kind: "branch", value: fixtureBranchName },
      opts: { autoCommit: true },
    } });
    expect(hasSuccessToast(`已合并 ${fixtureBranchName}。`)).toBe(true);
  });

  it("surfaces merge failure with a visible toast and the engine's message in diagnostics", async () => {
    const wrapper = await branchScreen();
    backend.failNext("merge_start", {
      code: "operation_in_progress",
      message: "已有合并操作正在进行。",
      detail: "MERGE_HEAD already exists",
      params: {},
      retryable: false,
      domain: "git",
    });
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, /^合并到/);
    await flushFlow(300);

    expectVerbFailure("merge_start", "已有合并操作正在进行。");
  });

  it("rebases the current branch onto the selected branch", async () => {
    const wrapper = await branchScreen();
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, "变基到");

    expect(backend.call("rebase_start")?.payload).toEqual({ args: {
      repo: fixtureRepoId,
      upstream: { kind: "branch", value: fixtureBranchName },
      onto: null,
      opts: {},
    } });
    expect(hasSuccessToast(`已变基到 ${fixtureBranchName}。`)).toBe(true);
  });

  it("shows rebase failure in a danger toast and diagnostics", async () => {
    const wrapper = await branchScreen();
    backend.failNext("rebase_start", {
      code: "checkout_conflicts",
      message: "变基因冲突而停止。",
      detail: "rebase stopped at a conflicting change",
      params: { branch: fixtureBranchName },
      retryable: false,
      domain: "git",
    });
    await selectBranch(wrapper, fixtureBranchName);
    await detailVerb(wrapper, "变基到");
    await flushFlow(300);

    expectVerbFailure("rebase_start", "变基因冲突而停止。");
  });

  it("checks out a local branch through the toolbar dialogue", async () => {
    const wrapper = await branchScreen();
    await chooseToolbarBranchOption(wrapper, "签出分支");
    await pickBranchInDialogue(fixtureBranchName);
    await confirmDialogue();
    await confirmDirtyIfAsked();

    expect(backend.call("branch_checkout")?.payload).toEqual({ args: { repo: fixtureRepoId, name: fixtureBranchName, opts: {} } });
    expect(hasSuccessToast("已签出")).toBe(true);
  });
});
