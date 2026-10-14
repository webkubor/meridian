/**
 * 失败可见性与诊断路径的流程回归
 * 路由：启动、工作区标签页、诊断页
 * API：真实 App.vue、Pinia、vue-i18n 与失败网，配可编程后端
 *
 * 每个用例都在这里是因为这些场景此前要么没有提示、要么提示了却无法定位：
 * 崩溃记录只存在于诊断页、后台加载失败被标记为“已处理”后只剩“尚无提交”
 * 这样的假陈述、切换分支后刷新失败导致推送了错误的分支、以及报告问题
 * 在浏览器拒绝链接时什么都不做。
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";

import {
  backend,
  clickToolbarVerb,
  confirmDialogue,
  clickWorkspaceTab,
  clipboard,
  dialogue,
  fixtureBranchName,
  flushFlow,
  hasDangerToast,
  hasSuccessToast,
  hasWarningToast,
  loggedToDiagnostics,
  mountApp,
  openRepositoryFixture,
  opener,
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

/** The dirty-tree confirmation, when the verb asked for one. */
async function confirmIfAsked(): Promise<void> {
  if (document.querySelector('[role="dialog"]')) await confirmDialogue();
}

/** Open the Diagnostics screen from the toolbar (a utility verb in every run). */
async function openDiagnostics(wrapper: VueWrapper): Promise<void> {
  const entry = wrapper.findAll(".utility-button").find((button) => button.attributes("title")?.includes("诊断"));
  if (!entry) throw new Error("The toolbar has no diagnostics entry");
  await entry.trigger("click");
  await flushFlow();
}

describe("失败在未渲染处也要看得见", () => {
  it("开启仓库时历史加载失败：工作区标签页不渲染历史面板，失败仍以提示出现", async () => {
    const wrapper = await mountApp();
    backend.setHandler("history_commits_load", () => {
      throw { code: "io", message: "读取历史失败。", detail: "index unreadable", params: {}, retryable: true, domain: "fs" };
    });
    backend.setHandler("history_graph_compute", () => ({ lanes: [], segments: [], laneCount: 0 }));
    await openRepositoryFixture(wrapper);
    await flushFlow(300);

    // 历史面板不在 DOM 中（当前是工作区标签页），所以没有人能"处理"这次失败。
    expect(hasDangerToast("读取历史失败。")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "history_commits_load")).toBe(true);
    expect(loggedToDiagnostics("history_commits_load")).toBe(true);

    // 侧栏不能说"尚无提交"——那是关于仓库的陈述，而失败不是。
    const sidebar = wrapper.find(".app-sidebar").text();
    expect(sidebar).not.toContain("尚无提交");
    expect(sidebar).toContain("文件操作失败。");

    // 切到历史标签页后，内联错误臂渲染的是同一句话。
    await clickWorkspaceTab("历史", wrapper);
    expect(wrapper.find(".list-error").text()).toContain("文件操作失败。");
    expect(wrapper.find(".list-error").text()).toContain("io");
  });

  it("开启仓库时分支列表失败：侧栏说加载失败，而不是“尚无分支”", async () => {
    const wrapper = await mountApp();
    backend.setHandler("branch_list", (payload) => {
      const filter = (payload?.args as { filter?: string } | undefined)?.filter;
      if (filter === "remote") return [];
      throw { code: "io", message: "读取分支失败。", detail: "refs unreadable", params: {}, retryable: true, domain: "fs" };
    });
    await openRepositoryFixture(wrapper);
    await flushFlow(300);

    expect(hasDangerToast("读取分支失败。")).toBe(true);
    const sidebar = wrapper.find(".app-sidebar").text();
    expect(sidebar).not.toContain("尚无分支");
    expect(sidebar).toContain("文件操作失败。");
  });

  it("启动时应用信息读取失败：没有界面渲染它，提示必须出现", async () => {
    await mountApp("/repos", () => {
      backend.setHandler("app_info_load", () => {
        throw { code: "io", message: "读取应用信息失败。", detail: "", params: {}, retryable: false, domain: "fs" };
      });
    });
    await flushFlow(300);

    expect(hasDangerToast("读取应用信息失败。")).toBe(true);
    expect(recordedFailures().some((failure) => failure.command === "app_info_load")).toBe(true);
  });

  it("启动时崩溃记录非空：横幅出现在任何界面之前，并可进入诊断或上报", async () => {
    const wrapper = await mountApp("/repos", () => {
      backend.setHandler("diagnostics_last_crash", () => ({
        at: "2026-10-14T09:00:00.000Z",
        message: "index out of bounds",
        location: "src/engine/history.rs:41",
        backtraceExcerpt: null,
        kind: "panic",
      }));
    });
    await flushFlow(300);

    const banner = wrapper.find(".crash-banner");
    expect(banner.exists()).toBe(true);
    expect(banner.text()).toContain("上次运行意外结束");
    expect(banner.text()).toContain("index out of bounds");

    const report = banner.findAll("button").find((button) => button.text().trim() === "报告问题");
    await report?.trigger("click");
    await flushFlow(300);
    // 上报先给出确认（含将要进入链接的正文），再打开浏览器。
    expect(dialogue().textContent).toContain("打开前请先确认报告内容");
    expect(opener.calls).toHaveLength(0);
  });

  it("启动时只有已恢复的内部错误：横幅说明它是内部错误，不是意外结束", async () => {
    const wrapper = await mountApp("/repos", () => {
      backend.setHandler("diagnostics_last_crash", () => ({
        at: "2026-10-14T09:00:00.000Z",
        message: "The command task panicked: index out of bounds",
        location: null,
        backtraceExcerpt: null,
        kind: "recovered_panic",
      }));
    });
    await flushFlow(300);

    expect(wrapper.find(".crash-banner").text()).toContain("有命令发生了内部错误");
    expect(wrapper.find(".crash-banner").text()).not.toContain("意外结束");
  });
});

describe("切换分支之后刷新失败", () => {
  it("警告一次并禁用推送，直到 HEAD 被重新读取", async () => {
    const wrapper = await repositoryScreen();
    backend.failNext("repository_refresh", { code: "io", message: "重新读取仓库失败。", detail: "", params: {}, retryable: true, domain: "fs" });
    await clickWorkspaceTab("分支", wrapper);
    const row = wrapper.findAll(".branch-row").find((candidate) => candidate.text().includes(fixtureBranchName));
    await row?.trigger("click");
    await flushFlow();
    const checkout = wrapper.findAll(".branch-detail button").find((button) => button.text().trim() === "签出");
    await checkout?.trigger("click");
    await confirmIfAsked();
    await flushFlow(300);

    // 签出成功了，但视图没跟上：必须说清楚，而不是让推送悄悄发布旧分支。
    expect(hasSuccessToast("已签出")).toBe(true);
    expect(hasWarningToast("分支已切换，但界面未能刷新。重新读取分支前，推送处于禁用状态。")).toBe(true);

    const push = wrapper.findAll(".toolbar button").find((button) => button.text().trim() === "推送");
    expect(push?.attributes("disabled")).toBeDefined();
    expect(push?.attributes("title")).toBe("未能重新读取当前分支，推送已禁用。");

    // 一次成功的刷新（抓取后整仓重读）之后，推送恢复。
    await clickToolbarVerb("抓取", wrapper);
    await flushFlow(300);
    const refreshed = wrapper.findAll(".toolbar button").find((button) => button.text().trim() === "推送");
    expect(refreshed?.attributes("disabled")).toBeUndefined();
  });
});

describe("合并因冲突停下", () => {
  it("不算成功：警告提示并跳到工作区", async () => {
    const wrapper = await repositoryScreen();
    backend.setHandler("merge_start", () => ({
      outcome: { kind: "conflicted" },
      conflicts: [{ path: "src/app.ts" }],
      notes: [],
    }));
    await clickWorkspaceTab("分支", wrapper);
    const row = wrapper.findAll(".branch-row").find((candidate) => candidate.text().includes(fixtureBranchName));
    await row?.trigger("click");
    await flushFlow();
    const merge = wrapper.findAll(".branch-detail button").find((button) => button.text().includes("合并到"));
    await merge?.trigger("click");
    await flushFlow(300);

    // `merge_start` returns Ok with `outcome: conflicted`: a success toast here
    // would say "merged" for an operation that is waiting for a human.
    expect(hasSuccessToast("已合并")).toBe(false);
    expect(hasWarningToast("因冲突而中止。请在工作区解决冲突后继续。")).toBe(true);
    expect(wrapper.find(".workspace-tab.active").text()).toContain("工作区");
  });
});

describe("报告问题", () => {
  it("先显示将要进入链接的正文；取消不打开任何链接，复制进入剪贴板", async () => {
    const wrapper = await repositoryScreen();
    backend.setHandler("app_feedback_compose", () => ({
      url: "https://github.com/example/meridian/issues/new?template=bug_report.yml&diagnostics=%5Bselected%20failure%5D%0Acode%3A%20io",
      prefilled: ["version: 0.1.0"],
    }));
    await openDiagnostics(wrapper);
    const report = wrapper.findAll("button").find((button) => button.text().trim() === "报告问题");
    await report?.trigger("click");
    await flushFlow(300);

    // 确认框展示解码后的正文：查询串在页面第一次加载时就到达 github.com，
    // 所以审阅必须发生在内容离开本机之前。
    const panel = dialogue().textContent ?? "";
    expect(panel).toContain("打开前请先确认报告内容");
    expect(panel).toContain("[selected failure]");
    expect(panel).toContain("code: io");
    expect(opener.calls).toHaveLength(0);

    const copy = [...dialogue().querySelectorAll("button")].find((button) => button.textContent?.trim() === "复制报告");
    expect(copy).toBeDefined();
    (copy as HTMLElement).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await flushFlow();
    expect(clipboard.writes.some((text) => text.includes("[selected failure]"))).toBe(true);
    expect(opener.calls).toHaveLength(0);
  });

  it("确认后打开链接；浏览器拒绝时复制地址并给出提示", async () => {
    const wrapper = await repositoryScreen();
    backend.setHandler("app_feedback_compose", () => ({
      url: "https://github.com/example/meridian/issues/new?diagnostics=%5Breport%5D",
      prefilled: [],
    }));
    await openDiagnostics(wrapper);
    const report = wrapper.findAll("button").find((button) => button.text().trim() === "报告问题");
    await report?.trigger("click");
    await flushFlow(300);

    const open = [...dialogue().querySelectorAll("button")].find((button) => button.textContent?.trim() === "打开 GitHub");
    (open as HTMLElement).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await flushFlow(300);
    expect(opener.calls).toEqual(["https://github.com/example/meridian/issues/new?diagnostics=%5Breport%5D"]);
    expect(hasSuccessToast("已在 GitHub 打开")).toBe(false);

    // 这一次浏览器拒绝：链接必须被复制并说明"没有发送任何内容"。
    opener.fail = true;
    const again = wrapper.findAll("button").find((button) => button.text().trim() === "报告问题");
    await again?.trigger("click");
    await flushFlow(300);
    const retry = [...dialogue().querySelectorAll("button")].find((button) => button.textContent?.trim() === "打开 GitHub");
    (retry as HTMLElement).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await flushFlow(300);

    expect(hasDangerToast("无法打开该链接。")).toBe(true);
    expect(clipboard.writes).toContain("https://github.com/example/meridian/issues/new?diagnostics=%5Breport%5D");
    // 失败本身被记录，而不是被 `false` 吞掉。
    expect(recordedFailures().some((failure) => failure.command === "opener.openUrl")).toBe(true);
  });
});

describe("应用自身的异常", () => {
  it("视图模型映射器抛错时也会留下记录、日志与提示", async () => {
    const wrapper = await repositoryScreen();
    // 后端返回的载荷缺少字段，映射器在同一个 try 内抛出：没有任何命令失败，
    // 所以桥接层看不到它——此前结果只有面板里一句英文原文。
    backend.setHandler("history_commit_load", () => ({ oid: "abc" }));
    await clickWorkspaceTab("历史", wrapper);
    const row = wrapper.findAll(".commit-row").find((candidate) => candidate.text().includes("Add fixture main branch"));
    await row?.trigger("click");
    await flushFlow(300);

    expect(recordedFailures().some((failure) => failure.command === "history_commit_load")).toBe(true);
    expect(loggedToDiagnostics("history_commit_load")).toBe(true);
    // The pane that renders the error claims it, so the toast is not needed;
    // what matters is that the raw JavaScript text is not all the user gets.
    expect(wrapper.text()).toContain("无法加载提交");
    expect(wrapper.text()).toContain("Cannot read properties of undefined");
    expect(recordedFailures().at(-1)?.error.code).toBe("unclassified");
  });

  it("后端以裸字符串拒绝时，启动页的提示仍然有内容", async () => {
    // Tauri's 参数反序列化错误是字符串，不是 ErrorPayload；直接存储原值时，
    // 提示框拿到的 message 是 undefined，于是什么都不显示。
    const wrapper = await mountApp();
    backend.setHandler("repository_open", () => {
      throw "invalid args `args` for command `repository_open`: invalid type";
    });
    await wrapper.get<HTMLInputElement>("input[aria-label='要打开的文件夹']").setValue("/tmp/meridian-flow");
    await wrapper.get("form").trigger("submit");
    await flushFlow(300);

    expect(wrapper.find(".launch-error").text()).toContain("出了点问题");
    expect(wrapper.find(".launch-error").text()).toContain("unclassified");
    expect(recordedFailures().some((failure) => failure.command === "repository_open")).toBe(true);
  });
});

describe("失败风暴", () => {
  it("超过五次时告知还有多少个，而不是静默丢弃", async () => {
    await repositoryScreen();
    // Seven unhandled rejections nothing renders: five toasts, then one toast
    // that says how many were kept off screen.
    for (let index = 0; index < 7; index += 1) {
      window.dispatchEvent(new PromiseRejectionEvent("unhandledrejection", { promise: Promise.resolve(1), reason: new Error(`burst ${index}`) }));
      await flushFlow(300);
    }

    await flushFlow(1_500);

    const shown = [...document.querySelectorAll(".toast.kind-danger")];
    expect(shown.length).toBeGreaterThanOrEqual(5);
    // Each toast names its own failure, so two identical sentences are not two
    // indistinguishable boxes.
    expect(new Set(shown.map((toast) => toast.textContent)).size).toBe(shown.length);
    expect([...document.querySelectorAll(".toast")].some((toast) => toast.textContent?.includes("2 个错误"))).toBe(true);
    // Every one reached the recorder: none exists only in a toast.
    expect(recordedFailures().filter((failure) => failure.command === "internal:window.unhandledrejection").length).toBe(7);
  });
});

describe("诊断页", () => {
  it("进入时重新读取日志尾部，而不是用启动时的那一份", async () => {
    const wrapper = await repositoryScreen();
    backend.setHandler("diagnostics_log_tail", () => "[ERROR] 打开仓库后发生的失败");
    await openDiagnostics(wrapper);
    await flushFlow(300);

    expect(backend.calls.filter((call) => call.name === "diagnostics_log_tail").length).toBeGreaterThanOrEqual(2);
    expect(wrapper.find(".log-tail").text()).toContain("打开仓库后发生的失败");
  });
});
