/**
 * 比较、历史分页、设置、更新、反馈与错误诊断入口的流程回归
 * 路由：/repo/:repoId、/settings、/about、/diagnostics
 * API：真实 App.vue、Pinia、i18n、hash router 与诊断失败网
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";

import {
  backend,
  clickWorkspaceTab,
  confirmDialogue,
  fixtureRepoId,
  flushFlow,
  hasDangerToast,
  mountApp,
  openRepositoryFixture,
  pickBranchInDialogue,
  recordedFailures,
  scrollListToEnd,
  unmountApp,
} from "./harness";

beforeEach(() => vi.useFakeTimers());
afterEach(async () => {
  await unmountApp();
  vi.useRealTimers();
});

async function openRepository(): Promise<VueWrapper> {
  const wrapper = await mountApp();
  await openRepositoryFixture(wrapper);
  return wrapper;
}

describe("application and diagnostics operations", () => {
  it("compares the upstream against HEAD, then swaps the two sides", async () => {
    const wrapper = await openRepository();
    await clickWorkspaceTab("比较", wrapper);
    await flushFlow();

    // Opening the tab seeds base = the branch's upstream, target = HEAD, so the
    // first thing a user sees is a real comparison rather than two empty pickers.
    const seeded = wrapper.findAll("button.ref-picker .ref-name").map((name) => name.text());
    expect(seeded[0]).toContain("origin/main");
    expect(seeded[1]).toContain("main");
    expect(backend.call("history_merge_base")?.payload).toEqual({
      args: { repo: fixtureRepoId, left: { kind: "branch", value: "refs/remotes/origin/main" }, right: { kind: "head" } },
    });
    const rangeCalls = backend.calls.filter((call) => call.name === "history_commits_load");
    expect(rangeCalls.some((call) => {
      const args = call.payload?.args as Record<string, unknown>;
      return args.repo === fixtureRepoId && args.limit === 200;
    })).toBe(true);

    await wrapper.get("button.swap").trigger("click");
    await flushFlow();
    const swapped = wrapper.findAll("button.ref-picker .ref-name").map((name) => name.text());
    expect(swapped[0]).toBe(seeded[1]);
    expect(swapped[1]).toBe(seeded[0]);
    expect(backend.call("history_merge_base", 1)?.payload).toEqual({
      args: { repo: fixtureRepoId, left: { kind: "head" }, right: { kind: "branch", value: "refs/remotes/origin/main" } },
    });
  });

  it("compares an explicitly picked branch against HEAD", async () => {
    const wrapper = await openRepository();
    await clickWorkspaceTab("比较", wrapper);
    await wrapper.findAll("button.ref-picker")[0]!.trigger("click");
    await flushFlow();
    await pickBranchInDialogue("feature/ready");
    await confirmDialogue();
    await flushFlow();

    expect(backend.call("history_merge_base", 1)?.payload).toEqual({
      args: { repo: fixtureRepoId, left: { kind: "branch", value: "refs/heads/feature/ready" }, right: { kind: "head" } },
    });
  });

  it("shows compare command failure in the pane and records it", async () => {
    const wrapper = await openRepository();
    await clickWorkspaceTab("比较", wrapper);
    backend.failNext("history_commits_load", {
      code: "unknown_revision",
      message: "比较引用无法解析。",
      detail: "refs/heads/feature/ready not found",
      params: { revision: "feature/ready" },
      retryable: false,
      domain: "git",
    });
    await wrapper.findAll("button.ref-picker")[0]!.trigger("click");
    await flushFlow();
    await pickBranchInDialogue("feature/ready");
    await confirmDialogue();
    await flushFlow(300);

    expect(wrapper.get(".list-error").text()).toContain("比较引用无法解析。");
    expect(recordedFailures().some((failure) => failure.command === "history_commits_load")).toBe(true);
  });

  it("loads the next history page when the list asks for more", async () => {
    const wrapper = await openRepository();
    await clickWorkspaceTab("历史", wrapper);
    await flushFlow();
    await scrollListToEnd(".virtual-commit-list");

    expect(backend.calls.some((call) => {
      if (call.name !== "history_commits_load") return false;
      const args = call.payload?.args as Record<string, unknown>;
      return args.repo === fixtureRepoId && args.from !== undefined;
    })).toBe(true);
    expect(wrapper.text()).toContain("Earlier fixture commit");
  });

  it("reports a history paging failure inline and retains it in diagnostics", async () => {
    const wrapper = await openRepository();
    backend.failNext("history_commits_load", {
      code: "io",
      message: "无法加载更多提交。",
      detail: "revwalk failed",
      params: {},
      retryable: true,
      domain: "git",
    });
    await clickWorkspaceTab("历史", wrapper);
    await flushFlow();
    await scrollListToEnd(".virtual-commit-list");
    await flushFlow(300);

    expect(wrapper.get(".list-error").text()).toContain("无法加载更多提交。");
    expect(recordedFailures().some((failure) => failure.command === "history_commits_load")).toBe(true);
  });

  it("persists theme and locale preferences", async () => {
    const wrapper = await mountApp();
    await wrapper.find(".utility-button[title='设置']").trigger("click");
    await flushFlow();
    await wrapper.findAll('[role="radio"]').find((radio) => radio.text() === "深色")!.trigger("click");
    await flushFlow();
    await wrapper.findAll('[role="radio"]').find((radio) => radio.text() === "English")!.trigger("click");
    await flushFlow();

    expect(JSON.parse(localStorage.getItem("meridian.settings") ?? "{}")["shell.appearance"]).toBe("dark");
    expect(localStorage.getItem("meridian.appearance")).toBe("dark");
    expect(localStorage.getItem("meridian.locale")).toBe("en");
    expect(document.documentElement.lang).toBe("en");
    expect(wrapper.text()).toContain("Appearance");
  });

  it("reports a failed manual update check instead of showing a blank panel", async () => {
    const wrapper = await mountApp();
    await wrapper.find(".utility-button[title='关于 Meridian']").trigger("click");
    await flushFlow();
    backend.failNext("app_update_check", {
      code: "io",
      message: "无法连接更新服务器。",
      detail: "request timed out",
      params: {},
      retryable: true,
      domain: "service",
    });
    const check = wrapper.findAll("button.action").find((button) => button.text().trim() === "检查更新");
    expect(check).toBeDefined();
    await check!.trigger("click");
    await flushFlow(300);

    expect(wrapper.text()).toContain("检查更新失败。");
    expect(wrapper.text()).toContain("无法连接更新服务器。");
    expect(recordedFailures().some((failure) => failure.command === "app_update_check")).toBe(true);
  });

  it("says so when the build cannot compose a feedback link", async () => {
    const wrapper = await mountApp();
    await wrapper.find(".utility-button[title='关于 Meridian']").trigger("click");
    await flushFlow();
    backend.failNext("app_feedback_compose", {
      code: "invalid_request",
      message: "This build does not know which project to report to.",
      detail: "repo slug is not configured",
      params: {},
      retryable: false,
      domain: "internal",
    });
    await wrapper.findAll("button.action").find((button) => button.text().trim() === "报告问题")!.trigger("click");
    await flushFlow(300);

    expect(wrapper.text()).toContain("无法生成报告链接。");
    expect(recordedFailures().some((failure) => failure.command === "app_feedback_compose")).toBe(true);
  });

  it("shows a failure nobody claimed as a toast and opens diagnostics from it", async () => {
    const wrapper = await mountApp();
    window.dispatchEvent(new PromiseRejectionEvent("unhandledrejection", { promise: Promise.resolve(1), reason: new Error("unexpected fixture rejection") }));
    await flushFlow(300);

    expect(recordedFailures().some((failure) => failure.command === "internal:window.unhandledrejection")).toBe(true);
    expect(hasDangerToast("发生内部错误。")).toBe(true);

    const details = [...document.querySelectorAll(".toast-action")].find((button) => button.textContent?.trim() === "详情");
    expect(details).toBeDefined();
    (details as HTMLElement).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await flushFlow();

    expect(window.location.hash).toBe("#/diagnostics");
    expect(wrapper.text()).toContain("诊断");
    expect(wrapper.text()).toContain("window.unhandledrejection");
  });

  it("keeps a launch-screen failure inline instead of double-reporting it as a toast", async () => {
    const wrapper = await mountApp();
    backend.rejectAll();
    await wrapper.get<HTMLInputElement>("input[aria-label='要打开的文件夹']").setValue("/tmp/meridian-flow");
    await wrapper.get("form").trigger("submit");
    await flushFlow(300);

    expect(wrapper.get(".launch-error[role='alert']").text()).toContain("repository_open rejected by the meta test");
    expect(recordedFailures().some((failure) => failure.command === "repository_open")).toBe(true);
    expect(wrapper.findAll(".kind-danger")).toHaveLength(0);
  });
});
