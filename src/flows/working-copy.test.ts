/**
 * 工作区暂存、撤销、放弃与提交的真实命令流
 * 路由：/repo/:repoId 的工作区、历史与比较页签
 * API：真实 App.vue 使用 Pinia stores 与 Tauri 命令桥接层
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";

import {
  backend,
  clickInDialogue,
  clickSelector,
  dialogue,
  fixtureFilePath,
  fixtureRepoId,
  flushFlow,
  hasDangerToast,
  hasSuccessToast,
  loggedToDiagnostics,
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

async function openWorkingCopy(): Promise<VueWrapper> {
  const wrapper = await mountApp();
  await openRepositoryFixture(wrapper);
  return wrapper;
}

function fileSection(wrapper: VueWrapper, title: string) {
  const section = wrapper.findAll(".file-section").find((candidate) => candidate.text().includes(title));
  if (!section) throw new Error(`Working-copy section ${JSON.stringify(title)} was not rendered`);
  return section;
}

function fileRow(wrapper: VueWrapper, path: string) {
  const row = wrapper.findAll(".file-row").find((candidate) => candidate.text().includes(path));
  if (!row) throw new Error(`Working-copy row ${JSON.stringify(path)} was not rendered`);
  return row;
}

/** Stage the fixture file and return the row as it looks afterwards. */
async function stageFixtureFile(wrapper: VueWrapper): Promise<void> {
  await fileRow(wrapper, fixtureFilePath).get('button[title="暂存"]').trigger("click");
  await flushFlow();
}

describe("working-copy operations", () => {
  it("stages one file and refreshes the visible staged count", async () => {
    const wrapper = await openWorkingCopy();
    await stageFixtureFile(wrapper);

    expect(backend.call("wc_stage_paths")?.payload).toEqual({ args: { repo: fixtureRepoId, paths: [fixtureFilePath] } });
    expect(wrapper.text()).toContain("暂存 1 个");
    expect(fileSection(wrapper, "暂存区").text()).toContain(fixtureFilePath);
  });

  it("keeps the file list on a stage failure, toasts the reason, and records it", async () => {
    const wrapper = await openWorkingCopy();
    backend.failNext("wc_stage_paths", {
      code: "index_locked",
      message: "索引正被另一个 Git 进程使用。",
      detail: "index.lock exists",
      params: { path: fixtureFilePath },
      retryable: true,
      domain: "git",
    });
    await fileRow(wrapper, fixtureFilePath).get('button[title="暂存"]').trigger("click");
    await flushFlow(300);

    // A failed mutation no longer replaces the list with an error arm: the
    // staged-set state the user is looking at is still true, and the failure
    // arrives as a toast with the engine's reason and a route into diagnostics.
    expect(wrapper.find(".file-row").exists()).toBe(true);
    expect(hasDangerToast("另一个 Git 进程正在使用该仓库")).toBe(true);
    expect(hasDangerToast("索引正被另一个 Git 进程使用。")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "wc_stage_paths")).toBe(true);
    // And it reached the local log, which is where a bug report is assembled.
    expect(loggedToDiagnostics()).toBe(true);
  });

  it("unstages one file and updates the list", async () => {
    const wrapper = await openWorkingCopy();
    await stageFixtureFile(wrapper);
    await fileRow(wrapper, fixtureFilePath).get('button[title="取消暂存"]').trigger("click");
    await flushFlow();

    expect(backend.call("wc_unstage_paths")?.payload).toEqual({ args: { repo: fixtureRepoId, paths: [fixtureFilePath] } });
    expect(fileSection(wrapper, "已修改").text()).toContain(fixtureFilePath);
  });

  it("keeps the list on an unstage failure, toasts it and records it", async () => {
    const wrapper = await openWorkingCopy();
    await stageFixtureFile(wrapper);
    backend.failNext("wc_unstage_paths", {
      code: "io",
      message: "无法从暂存区移除文件。",
      detail: "reset_default failed",
      params: {},
      retryable: false,
      domain: "git",
    });
    await fileRow(wrapper, fixtureFilePath).get('button[title="取消暂存"]').trigger("click");
    await flushFlow(300);

    expect(wrapper.find(".file-row").exists()).toBe(true);
    expect(hasDangerToast("无法从暂存区移除文件。")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "wc_unstage_paths")).toBe(true);
  });

  it("bulk-stages every modified file with one exact paths payload", async () => {
    const wrapper = await openWorkingCopy();
    await fileSection(wrapper, "已修改").get("button.section-bulk").trigger("click");
    await flushFlow();

    expect(backend.call("wc_stage_paths")?.payload).toEqual({ args: { repo: fixtureRepoId, paths: [fixtureFilePath] } });
    expect(wrapper.text()).toContain("暂存 1 个");
  });

  it("sends the whole selection in one command, so a failure cannot be erased by a later success", async () => {
    // Two modified files, so the section's bulk control has two paths to send:
    // one call per path raced, and a success landing after the failure
    // overwrote the error state, leaving a file silently unstaged.
    const wrapper = await mountApp();
    backend.setEntries([
      { path: "src/app.ts", code: "modified", staged: false, assumeUnchanged: false, view: { short: "M", text: "modified", tone: "info" } },
      { path: "src/other.ts", code: "modified", staged: false, assumeUnchanged: false, view: { short: "M", text: "modified", tone: "info" } },
    ]);
    await openRepositoryFixture(wrapper);
    backend.failNext("wc_stage_paths", {
      code: "io",
      message: "无法暂存所选文件。",
      detail: "index write failed",
      params: {},
      retryable: true,
      domain: "git",
    });
    await fileSection(wrapper, "已修改").get("button.section-bulk").trigger("click");
    await flushFlow(300);

    const stageCalls = backend.calls.filter((call) => call.name === "wc_stage_paths");
    expect(stageCalls).toHaveLength(1);
    expect((stageCalls[0]?.payload as { args: { paths: string[] } }).args.paths).toHaveLength(2);
    // The list is still on screen and the failure is in the user's face.
    expect(wrapper.findAll(".file-row").length).toBeGreaterThan(0);
    expect(hasDangerToast("无法暂存所选文件。")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "wc_stage_paths")).toBe(true);
  });

  it("confirms discard before sending the destructive command", async () => {
    const wrapper = await openWorkingCopy();
    await fileRow(wrapper, fixtureFilePath).get('button[title="放弃改动"]').trigger("click");
    await flushFlow();

    expect(backend.call("wc_paths_discard")).toBeUndefined();
    expect(dialogue().textContent).toContain(fixtureFilePath);
    await clickInDialogue("button.btn.danger");

    expect(backend.call("wc_paths_discard")?.payload).toEqual({ args: { repo: fixtureRepoId, paths: [fixtureFilePath], opts: {} } });
    expect(hasSuccessToast("已放弃该文件的改动。")).toBe(true);
    expect(wrapper.text()).not.toContain(fixtureFilePath);
  });

  it("cancelling the discard dialogue sends nothing and reports no failure", async () => {
    const wrapper = await openWorkingCopy();
    await fileRow(wrapper, fixtureFilePath).get('button[title="放弃改动"]').trigger("click");
    await flushFlow();
    await clickInDialogue(".dialogue-close");

    expect(backend.call("wc_paths_discard")).toBeUndefined();
    expect(wrapper.text()).toContain(fixtureFilePath);
    expect(recordedFailures()).toHaveLength(0);
  });

  it("surfaces a discard failure after confirmation", async () => {
    const wrapper = await openWorkingCopy();
    backend.failNext("wc_paths_discard", {
      code: "io",
      message: "无法放弃文件改动。",
      detail: "checkout_index failed",
      params: { path: fixtureFilePath },
      retryable: false,
      domain: "git",
    });
    await fileRow(wrapper, fixtureFilePath).get('button[title="放弃改动"]').trigger("click");
    await flushFlow();
    await clickInDialogue("button.btn.danger");
    await flushFlow(300);

    expect(hasDangerToast("无法放弃文件改动。")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "wc_paths_discard")).toBe(true);
  });

  it("creates a commit with the message and refreshes history", async () => {
    const wrapper = await openWorkingCopy();
    await stageFixtureFile(wrapper);
    const subject = wrapper.get<HTMLInputElement>("input[aria-label='提交标题']");
    await subject.setValue("Flow fixture commit");
    await flushFlow();
    await clickSelector(".composer-foot .split-primary", wrapper);

    expect(backend.call("commit_create")?.payload).toEqual({ args: { repo: fixtureRepoId, message: "Flow fixture commit", opts: { signOff: false } } });
    expect(hasSuccessToast("提交完成。")).toBe(true);
    expect(backend.calls.filter((call) => call.name === "history_commits_load").length).toBeGreaterThan(1);
  });

  it("shows a commit failure in a danger toast and records it", async () => {
    const wrapper = await openWorkingCopy();
    await stageFixtureFile(wrapper);
    backend.failNext("commit_create", {
      code: "io",
      message: "提交钩子拒绝了这次提交。",
      detail: "pre-commit exited with status 1",
      params: {},
      retryable: false,
      domain: "git",
    });
    await wrapper.get<HTMLInputElement>("input[aria-label='提交标题']").setValue("Flow fixture commit");
    await flushFlow();
    await clickSelector(".composer-foot .split-primary", wrapper);
    await flushFlow(300);

    expect(hasDangerToast("提交钩子拒绝了这次提交。")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "commit_create")).toBe(true);
  });

  it("amends the last commit through the amend switch and command", async () => {
    const wrapper = await openWorkingCopy();
    await stageFixtureFile(wrapper);
    await clickSelector('.composer-toggles button[role="switch"]', wrapper);
    expect(wrapper.get(".composer-title").text()).toBe("修订提交");
    await wrapper.get<HTMLInputElement>("input[aria-label='提交标题']").setValue("Amended fixture commit");
    await flushFlow();
    await clickSelector(".composer-foot .split-primary", wrapper);

    expect(backend.call("commit_amend")?.payload).toEqual({ args: { repo: fixtureRepoId, message: "Amended fixture commit", opts: { signOff: false } } });
    expect(backend.call("commit_create")).toBeUndefined();
    expect(hasSuccessToast("提交完成。")).toBe(true);
  });

  it("shows the amend failure in a danger toast", async () => {
    const wrapper = await openWorkingCopy();
    await stageFixtureFile(wrapper);
    backend.failNext("commit_amend", {
      code: "unknown_revision",
      message: "无法修订当前提交。",
      detail: "HEAD does not resolve to a commit",
      params: { revision: "HEAD" },
      retryable: false,
      domain: "git",
    });
    await clickSelector('.composer-toggles button[role="switch"]', wrapper);
    await wrapper.get<HTMLInputElement>("input[aria-label='提交标题']").setValue("Amended fixture commit");
    await flushFlow();
    await clickSelector(".composer-foot .split-primary", wrapper);
    await flushFlow(300);

    expect(hasDangerToast("找不到修订：HEAD")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "commit_amend")).toBe(true);
  });

});
