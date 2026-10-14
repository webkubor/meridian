/**
 * 仓库打开、浏览、创建、克隆与附近扫描的端到端命令流
 * 路由：/repos 与 /repo/:repoId
 * API：真实 App.vue 通过 Tauri 命令桥接层调用可编程 fake backend
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { alertText, backend, clickButton, clickSelector, flushFlow, fixtureRepoId, fixtureRepoPath, mountApp, recordedFailures, unmountApp } from "./harness";

beforeEach(() => vi.useFakeTimers());
afterEach(async () => {
  await unmountApp();
  vi.useRealTimers();
});

describe("repository routine operations", () => {
  it("opens a typed path with the exact wire payload and navigates into the repository", async () => {
    const wrapper = await mountApp();
    await wrapper.get<HTMLInputElement>("input[aria-label='要打开的文件夹']").setValue(fixtureRepoPath);
    await wrapper.get("form").trigger("submit");
    await flushFlow();

    expect(backend.call("repository_open")?.payload).toEqual({ args: { path: fixtureRepoPath, sender: "dialogue" } });
    expect(window.location.hash).toBe(`#/${"repo"}/${encodeURIComponent(fixtureRepoId)}`);
    expect(wrapper.text()).toContain("工作区");
  });

  it("browse fills the path from the picker, and cancelling leaves it unchanged without a failure", async () => {
    const wrapper = await mountApp();
    backend.pickNext("/tmp/picked-workspace");
    await clickSelector(".launch-browse", wrapper);
    expect(backend.call("app_pick_directory")?.payload).toBeUndefined();
    expect(wrapper.get<HTMLInputElement>("input[aria-label='要打开的文件夹']").element.value).toBe("/tmp/picked-workspace");

    backend.pickNext(null);
    await clickSelector(".launch-browse", wrapper);
    expect(wrapper.get<HTMLInputElement>("input[aria-label='要打开的文件夹']").element.value).toBe("/tmp/picked-workspace");
    expect(recordedFailures()).toHaveLength(0);
  });

  it("shows a realistic open failure inline and records it without an unhandled rejection", async () => {
    const wrapper = await mountApp();
    const payload = {
      code: "invalid_repository" as const,
      message: "这个文件夹不是 Git 仓库。",
      detail: "repository_open could not discover a repository",
      params: { path: fixtureRepoPath },
      retryable: false,
      domain: "git" as const,
    };
    backend.failNext("repository_open", payload);
    await wrapper.get<HTMLInputElement>("input[aria-label='要打开的文件夹']").setValue(fixtureRepoPath);
    await wrapper.get("form").trigger("submit");
    await flushFlow(300);

    expect(alertText()).toContain(payload.message);
    expect(recordedFailures().some((failure) => failure.command === "repository_open" && failure.error.message === payload.message)).toBe(true);
    expect(window.location.hash).toBe("#/repos");
  });

  it("creates a repository and opens it with the exact options", async () => {
    const wrapper = await mountApp();
    await clickButton("创建", wrapper);
    await wrapper.get<HTMLInputElement>("input[aria-label='新仓库所在的文件夹']").setValue("/tmp/new-flow-repo");
    await wrapper.get("form").trigger("submit");
    await flushFlow();

    expect(backend.call("repository_init")?.payload).toEqual({ args: { path: "/tmp/new-flow-repo", createGitignore: true } });
    expect(window.location.hash).toContain("/repo/");
  });

  it("shows an init error inline and records the rejected command", async () => {
    const wrapper = await mountApp();
    backend.failNext("repository_init", {
      code: "io",
      message: "无法创建仓库目录。",
      detail: "permission denied",
      params: { path: "/tmp/new-flow-repo" },
      retryable: false,
      domain: "fs",
    });
    await clickButton("创建", wrapper);
    await wrapper.get<HTMLInputElement>("input[aria-label='新仓库所在的文件夹']").setValue("/tmp/new-flow-repo");
    await wrapper.get("form").trigger("submit");
    await flushFlow(300);

    expect(alertText()).toContain("无法创建仓库目录。");
    expect(recordedFailures().some((failure) => failure.command === "repository_init")).toBe(true);
  });

  it("clones after destination suggestion and picker confirmation", async () => {
    const wrapper = await mountApp();
    backend.pickNext("/tmp/clones");
    await clickButton("克隆", wrapper);
    await wrapper.get<HTMLInputElement>("input[aria-label='要克隆的仓库地址']").setValue("https://example.invalid/team/demo.git");
    await wrapper.get("form").trigger("submit");
    await flushFlow();

    expect(backend.call("repository_clone_suggest_destination")?.payload).toEqual({ url: "https://example.invalid/team/demo.git" });
    expect(backend.call("app_pick_directory")?.payload).toBeUndefined();
    expect(backend.call("repository_clone")?.payload).toEqual({ args: { url: "https://example.invalid/team/demo.git", destination: "/tmp/clones/fixture-repository" } });
    expect(window.location.hash).toContain("/repo/");
  });

  it("surfaces a clone error and records it", async () => {
    const wrapper = await mountApp();
    backend.pickNext("/tmp/clones");
    backend.failNext("repository_clone", {
      code: "io",
      message: "克隆失败：无法连接远程仓库。",
      detail: "network unavailable",
      params: {},
      retryable: true,
      domain: "git",
    });
    await clickButton("克隆", wrapper);
    await wrapper.get<HTMLInputElement>("input[aria-label='要克隆的仓库地址']").setValue("https://example.invalid/team/demo.git");
    await wrapper.get("form").trigger("submit");
    await flushFlow(300);

    expect(alertText()).toContain("克隆失败：无法连接远程仓库。");
    expect(recordedFailures().some((failure) => failure.command === "repository_clone")).toBe(true);
  });

  it("scans nearby repositories after folder selection and renders the result", async () => {
    const wrapper = await mountApp();
    backend.pickNext("/tmp/search-root");
    await clickButton("重新扫描", wrapper);
    await flushFlow();

    expect(backend.call("repository_discover")?.payload).toEqual({ args: { root: "/tmp/search-root" } });
    expect(wrapper.text()).toContain("nearby-fixture");
  });

  it("shows a nearby-scan failure as an inline error on the launch screen and records it", async () => {
    const wrapper = await mountApp();
    backend.pickNext("/tmp/search-root");
    backend.failNext("repository_discover", {
      code: "io",
      message: "扫描此文件夹失败。",
      detail: "access denied",
      params: { root: "/tmp/search-root" },
      retryable: false,
      domain: "fs",
    });
    await clickButton("重新扫描", wrapper);
    await flushFlow(300);

    expect(alertText()).toContain("扫描此文件夹失败。");
    expect(recordedFailures().some((failure) => failure.command === "repository_discover")).toBe(true);
  });

  it("opens a nearby repository as a distinct path-based operation", async () => {
    const wrapper = await mountApp();
    backend.pickNext("/tmp/search-root");
    await clickButton("重新扫描", wrapper);
    await flushFlow();
    await clickSelector(".nearby-row", wrapper);
    await flushFlow();

    expect(backend.call("repository_open")?.payload).toEqual({ args: { path: "/tmp/nearby-fixture", sender: "dialogue" } });
    expect(window.location.hash).toContain("/repo/");
  });
});
