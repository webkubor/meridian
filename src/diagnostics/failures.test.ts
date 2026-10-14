/**
 * The guarantee: a failed operation is never silent, and is never noisy twice.
 *
 * Every test here drives a fake clock and a fake scheduler so the grace period,
 * the 2 s de-duplication and the 10 s cap are checked as arithmetic rather than
 * by waiting.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { reactive } from "vue";

import type { ErrorPayload } from "../bridges/bindings";
import { createFailureNet, normalizeError, reportFailure } from "./failures";
import { clearRecorder, recentFailures } from "./recorder";

function gitError(overrides: Partial<ErrorPayload> = {}): ErrorPayload {
  return { code: "index_locked", message: "Another Git process is using this repository.", retryable: true, domain: "git", ...overrides };
}

/** A net whose timer only fires when the test says so. */
function harness() {
  let clock = 1_000;
  const toasts: ErrorPayload[] = [];
  const timers: Array<() => void> = [];
  const net = createFailureNet({
    graceMs: 250,
    now: () => clock,
    schedule: (callback) => {
      timers.push(callback);
      return timers.length - 1;
    },
    cancel: (handle) => {
      timers[handle as number] = () => {};
    },
    showToast: (payload) => toasts.push(payload),
  });
  return {
    net,
    toasts,
    advance: (ms: number) => {
      clock += ms;
    },
    flush: () => {
      const due = timers.splice(0, timers.length);
      for (const callback of due) callback();
    },
  };
}

beforeEach(() => clearRecorder());

describe("the failure net", () => {
  it("shows exactly one fallback toast for a rejection nobody handled", () => {
    const { net, toasts, flush } = harness();
    const error = gitError();

    net.capture(error, "wc_status_load");
    expect(toasts).toEqual([]); // still inside the grace period
    flush();

    expect(toasts).toHaveLength(1);
    expect(toasts[0]?.code).toBe("index_locked");
    // Flushing again must not resurrect it: the pending entry is gone.
    flush();
    expect(toasts).toHaveLength(1);
  });

  it("shows nothing when the caller marks the failure handled in time", () => {
    const { net, toasts, flush } = harness();
    const error = gitError();

    net.capture(error, "wc_status_load");
    net.markHandled(error);
    flush();

    expect(toasts).toEqual([]);
  });

  it("treats a failure captured as handled as never pending at all", () => {
    const { net, toasts, flush } = harness();
    net.capture(gitError(), "wc_status_load", { handled: true });
    flush();
    expect(toasts).toEqual([]);
  });

  it("still records a handled failure, so the screen can list it", () => {
    const { net } = harness();
    net.capture(gitError(), "wc_status_load", { handled: true });
    expect(recentFailures()).toHaveLength(1);
  });

  it("matches a Vue-reactive copy of the error back to its pending entry", () => {
    // Stores hand `lastError.value` to the shell, which is a reactive proxy of
    // the same object; claiming it must silence the toast it belongs to.
    const { net, toasts, flush } = harness();
    const error = gitError();
    const proxy = reactive(error);
    net.capture(error, "wc_status_load");
    net.markHandled(proxy);
    flush();
    expect(toasts).toEqual([]);
  });

  it("de-duplicates an identical message inside two seconds", () => {
    const { net, toasts, flush, advance } = harness();
    net.capture(gitError(), "wc_status_load");
    net.capture(gitError(), "wc_status_load");
    advance(100);
    flush();
    expect(toasts).toHaveLength(1);
  });

  it("shows the same message again once two seconds have passed", () => {
    const { net, toasts, flush, advance } = harness();
    net.capture(gitError(), "wc_status_load");
    flush();
    advance(2_001);
    net.capture(gitError(), "wc_status_load");
    flush();
    expect(toasts).toHaveLength(2);
  });

  it("caps the burst at five toasts in ten seconds while still recording the rest", () => {
    const { net, toasts, flush } = harness();
    for (let index = 0; index < 8; index += 1) {
      net.capture(gitError({ code: "io", message: `Failure number ${index}` }), "wc_status_load");
    }
    flush();
    expect(toasts).toHaveLength(5);
    // The three that were suppressed reached the recorder; the user is told.
    expect(recentFailures().length).toBeGreaterThanOrEqual(8);
  });

  it("keeps two failures that merely share a sentence", () => {
    // The same words under different codes are different events: "Meridian
    // refused that request" covers invalid_request and auth_required alike.
    const { net, toasts, flush } = harness();
    net.capture(gitError({ code: "index_locked", message: "Refused." }), "wc_status_load");
    net.capture(gitError({ code: "io", message: "Refused." }), "wc_stage_paths");
    flush();
    expect(toasts).toHaveLength(2);
  });

  it("treats the same primitive again after the window as a new failure", () => {
    // A value-keyed cache made every second unhandled rejection of the same
    // string invisible: no record, no log line, no toast.
    const { net, toasts, flush, advance } = harness();
    net.capture("boom", "internal:window.unhandledrejection");
    flush();
    advance(10_000);
    net.capture("boom", "internal:window.unhandledrejection");
    flush();
    expect(toasts).toHaveLength(2);
    expect(recentFailures().filter((failure) => failure.command === "internal:window.unhandledrejection")).toHaveLength(2);
  });

  it("keeps a failure pending until a toast sink exists", () => {
    // A failure raised during bootstrap (before App.vue mounts) must not be
    // dropped: it is shown as soon as the sink is configured.
    const net = createFailureNet({ graceMs: 0, schedule: (callback) => { callback(); return 0; }, cancel: () => {} });
    net.capture(gitError(), "wc_status_load");
    const shown: ErrorPayload[] = [];
    net.configure({ showToast: (payload) => shown.push(payload) });
    expect(shown).toHaveLength(1);
  });

  it("reports a repeated rejection of the same object only once", () => {
    const { net, flush } = harness();
    const error = gitError();
    net.capture(error, "wc_status_load");
    net.capture(error, "wc_status_load");
    flush();
    expect(recentFailures()).toHaveLength(1);
  });

  it("records a fresh failure for a fresh call, even with the same shape", () => {
    const { net, flush } = harness();
    net.capture(gitError(), "wc_status_load", { newCall: true });
    net.capture(gitError(), "wc_status_load", { newCall: true });
    flush();
    expect(recentFailures()).toHaveLength(2);
  });

  it("never throws when the toast host itself throws", () => {
    const net = createFailureNet({
      graceMs: 0,
      schedule: (callback) => {
        callback();
        return 0;
      },
      cancel: () => {},
      showToast: () => {
        throw new Error("toast host is broken");
      },
    });
    expect(() => net.capture(gitError(), "wc_status_load")).not.toThrow();
  });
});

describe("normalizeError", () => {
  it("passes a wire payload through field for field", () => {
    const wire = gitError({ detail: "index.lock exists", params: { path: "/tmp/repo" } });
    expect(normalizeError(wire)).toEqual(wire);
  });

  it("gives a non-structured rejection a code and keeps its text as detail", () => {
    const payload = normalizeError("something exploded");
    expect(payload.code).toBe("unclassified");
    expect(payload.domain).toBe("internal");
    expect(payload.detail).toBe("something exploded");
  });

  it("keeps an Error's message for the report", () => {
    expect(normalizeError(new Error("bad things")).detail).toContain("bad things");
  });

  it("survives a value whose own toString throws", () => {
    const hostile = { toString: () => { throw new Error("no"); } };
    expect(normalizeError(hostile).code).toBe("unclassified");
  });

  it("keeps the code and the message of a payload from a newer backend", () => {
    // Flattening it to "[object Object]" lost the message from the toast, the
    // log write and the report; the domain falls back to internal instead.
    const payload = normalizeError({ code: "io", message: "disk full", domain: "network" });
    expect(payload.code).toBe("io");
    expect(payload.message).toBe("disk full");
    expect(payload.domain).toBe("internal");
  });

  it("keeps an unrecognised plugin error's content as JSON, not [object Object]", () => {
    const payload = normalizeError({ plugin: "opener", reason: "no handler" });
    expect(payload.detail).toContain("opener");
    expect(payload.detail).toContain("no handler");
    expect(payload.detail).not.toContain("[object Object]");
  });

  it("survives a circular object", () => {
    const circular: Record<string, unknown> = { a: 1 };
    circular.self = circular;
    expect(normalizeError(circular).detail).toContain("circular");
  });
});

describe("reportFailure / claimFailure", () => {
  beforeEach(() => clearRecorder());

  it("records an exception thrown by the app's own code", () => {
    // A view-model mapper inside a store's try: no command failed, so the
    // bridge never saw it and nothing was recorded anywhere. This is the
    // owner's complaint — a bug with no error shown and no trace.
    const payload = reportFailure(new TypeError("Cannot read properties of undefined (reading 'map')"), "history.loadDetail");
    expect(payload.code).toBe("unclassified");
    expect(payload.detail).toContain("Cannot read properties of undefined");
    expect(recentFailures().map((failure) => failure.command)).toContain("history.loadDetail");
  });

  it("normalises a bare string rejection into something renderable", () => {
    const payload = reportFailure("invalid args `args` for command `repository_open`", "repository_open");
    expect(payload.message).toBeTruthy();
    expect(payload.detail).toContain("invalid args");
    expect(recentFailures().at(-1)?.error.code).toBe("unclassified");
  });

  it("hands the caller a payload it can put straight into state", () => {
    const payload = reportFailure({ code: "io", message: "disk full", domain: "network" }, "wc_status_load");
    expect(payload).toEqual({
      code: "io",
      message: "disk full",
      detail: undefined,
      params: undefined,
      retryable: false,
      domain: "internal",
    });
  });
});
