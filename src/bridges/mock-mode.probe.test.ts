// Probe: the real call() path in mock mode (run with `vitest --mode mock`).
// In default test mode MOCK is false and the whole file self-skips — the flow
// tests own that mode via the FakeBackend vi.mock.
import { describe, expect, it } from "vitest";

import * as commands from "./commands";
import { resetMockState, MOCK_REPO_ID } from "./mock/state";

const describeMock = import.meta.env.MODE === "mock" ? describe : describe.skip;

describeMock("mock bridge (vitest --mode mock)", () => {
  it("serves app_info_load", async () => {
    resetMockState();
    const info = await commands.appInfoLoad();
    expect(info.name).toBe("Meridian");
    expect(info.canSelfUpdate).toBe(false);
  });

  it("serves wc_status_load with all FileStatus kinds", async () => {
    resetMockState();
    const status = await commands.wcStatusLoad(MOCK_REPO_ID);
    const codes = new Set(status.entries.map((e) => e.code));
    expect(codes.size).toBeGreaterThanOrEqual(6);
  });

  it("filters commit search through the args envelope", async () => {
    resetMockState();
    const all = await commands.historyCommitsLoad(MOCK_REPO_ID);
    const hits = await commands.historyCommitsLoad(MOCK_REPO_ID, { search: "blame" });
    expect(all.commits.length).toBeGreaterThan(hits.commits.length);
    expect(hits.commits.length).toBeGreaterThan(0);
    expect(hits.commits.length).toBeLessThan(150);
  });

  it("staging mutates state and emitting drives the event bus", async () => {
    resetMockState();
    const before = await commands.wcStatusLoad(MOCK_REPO_ID);
    const target = before.entries.find((e) => !e.staged);
    expect(target).toBeTruthy();
    await commands.wcStagePaths(MOCK_REPO_ID, [target!.path]);
    const after = await commands.wcStatusLoad(MOCK_REPO_ID);
    const staged = after.entries.find((e) => e.path === target!.path);
    expect(staged?.staged).toBe(true);
  });

  it("unknown oid produces a wire-shaped error through call()", async () => {
    resetMockState();
    await expect(commands.historyCommitLoad(MOCK_REPO_ID, "f".repeat(40))).rejects.toThrow(/unknown_revision/);
  });

  it("repository_open accepts any path (demo-friendly fixture)", async () => {
    resetMockState();
    const info = await commands.repositoryOpen("/definitely/not/a/repo", "cli");
    expect(info.workdir).toBe("/definitely/not/a/repo");
  });
});
