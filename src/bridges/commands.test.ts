/**
 * The bridge is where a failure becomes visible, so these tests pin its
 * contract: the caller receives the *same* rejection it would have received
 * without instrumentation, the failure is recorded once, and the diagnostic
 * commands can never report themselves in a loop.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class Channel<T> {
    onmessage: ((value: T) => void) | null = null;
  },
}));

import { Channel, invoke } from "@tauri-apps/api/core";
import type { ErrorPayload } from "./bindings";
import * as commands from "./commands";
import { configureFailureNet, markHandled } from "../diagnostics/failures";
import { clearRecorder, recentCalls, recentFailures } from "../diagnostics/recorder";

const invokeMock = vi.mocked(invoke);

const indexLocked: ErrorPayload = {
  code: "index_locked",
  message: "Another Git process is using this repository.",
  retryable: true,
  domain: "git",
};

beforeEach(() => {
  invokeMock.mockReset();
  clearRecorder();
});

describe("the invoke wrapper", () => {
  it("rethrows the identical rejection value", async () => {
    invokeMock.mockRejectedValueOnce(indexLocked);
    // Same reference, not a copy: a caller comparing or re-inspecting the error
    // must see what the backend actually sent.
    await expect(commands.wcStatusLoad("repo-1")).rejects.toBe(indexLocked);
  });

  it("records the failed call with its code and domain", async () => {
    invokeMock.mockRejectedValueOnce(indexLocked);
    await commands.wcStatusLoad("repo-1").catch(() => undefined);

    const failure = recentFailures().at(-1);
    expect(failure?.command).toBe("wc_status_load");
    expect(failure?.error.code).toBe("index_locked");
    expect(failure?.error.domain).toBe("git");
    // A second `diagnostics_log_write` follows the failure; the call ring holds both.
    expect(recentCalls().map((call) => call.command)).toContain("wc_status_load");
    expect(recentCalls().find((call) => call.command === "wc_status_load")?.ok).toBe(false);
  });

  it("forwards the failure to the log exactly once", async () => {
    invokeMock.mockRejectedValue(indexLocked);
    await commands.wcStatusLoad("repo-1").catch(() => undefined);

    const logWrites = invokeMock.mock.calls.filter(([name]) => name === "diagnostics_log_write");
    expect(logWrites).toHaveLength(1);
    expect(logWrites[0]?.[1]).toMatchObject({ args: { level: "error", source: "wc_status_load" } });
  });

  it("never logs a diagnostics command's own failure back into the log", async () => {
    invokeMock.mockRejectedValue(indexLocked);
    await commands.diagnosticsLogTail(50).catch(() => undefined);

    // One call, no follow-up write: the failure of the reporter is not reported.
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("diagnostics_log_tail", { args: { lines: 50 } });
  });

  it("stops after one log attempt when the log command itself fails", async () => {
    invokeMock.mockRejectedValue(indexLocked);
    await commands.wcStatusLoad("repo-1").catch(() => undefined);

    // The failing status load, then one attempt to log it — and no third call.
    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(recentFailures().map((failure) => failure.command)).toEqual(["wc_status_load", "diagnostics_log_write"]);
  });

  it("keeps argument key names, and values only in debug mode", async () => {
    invokeMock.mockResolvedValueOnce({ entries: [], summary: { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 }, repo: "repo-1" });
    await commands.wcStagePaths("repo-1", ["a.txt"]);
    const recorded = recentCalls().at(-1);
    expect(recorded?.argKeys).toEqual(["paths", "repo"]);
    expect(recorded?.args).toBeUndefined();
  });

  it("still hands the install channel through untouched", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    const seen: string[] = [];
    await commands.appUpdateInstall((progress) => seen.push(progress.phase));

    const lastCall = invokeMock.mock.calls.at(-1);
    const payload = lastCall?.[1] as { onProgress?: unknown } | undefined;
    expect(lastCall?.[0]).toBe("app_update_install");
    expect(payload?.onProgress).toBeInstanceOf(Channel);
  });

  it("sends the Rust remote_fetch mode, not the repository fetch-preference enum", async () => {
    invokeMock.mockResolvedValue(undefined as never);
    await commands.remoteFetch("repo-1", "origin");

    expect(invokeMock).toHaveBeenCalledWith("remote_fetch", {
      args: { repo: "repo-1", remote: "origin", refspecs: [], mode: "all" },
    });
  });

  it("sends the diagnostics commands to the names the Rust side registers", async () => {
    invokeMock.mockResolvedValue(undefined as never);
    await commands.diagnosticsDebugSet(true);
    await commands.diagnosticsExport("report text");
    expect(invokeMock).toHaveBeenCalledWith("diagnostics_debug_set", { args: { enabled: true } });
    expect(invokeMock).toHaveBeenCalledWith("diagnostics_export", { args: { context: "report text" } });
  });
});

/**
 * The end-to-end form of the guarantee: the bridge and the failure net wired to
 * each other, exactly as the app wires them, driven by a manual clock.
 */
describe("the bridge and the failure net together", () => {
  function wire() {
    const toasts: ErrorPayload[] = [];
    const timers: Array<() => void> = [];
    configureFailureNet({
      graceMs: 250,
      schedule: (callback) => {
        timers.push(callback);
        return timers.length - 1;
      },
      cancel: (handle) => {
        timers[handle as number] = () => {};
      },
      showToast: (payload) => toasts.push(payload),
    });
    return { toasts, flush: () => timers.splice(0, timers.length).forEach((callback) => callback()) };
  }

  it("produces exactly one fallback toast for a rejection nobody handles", async () => {
    const { toasts, flush } = wire();
    invokeMock.mockImplementation((name) => name === "wc_status_load" ? Promise.reject(indexLocked) : Promise.resolve(undefined));

    await commands.wcStatusLoad("repo-1").catch(() => undefined);
    expect(toasts).toEqual([]);
    flush();

    expect(toasts).toHaveLength(1);
    expect(toasts[0]?.code).toBe("index_locked");
  });

  it("produces none when the caller marks it handled inside the grace period", async () => {
    const { toasts, flush } = wire();
    invokeMock.mockImplementation((name) => name === "wc_status_load" ? Promise.reject(indexLocked) : Promise.resolve(undefined));

    await commands.wcStatusLoad("repo-1").catch(() => undefined);
    markHandled(indexLocked);
    flush();

    expect(toasts).toEqual([]);
  });
});
