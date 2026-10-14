/**
 * Global handlers are the last net under the bridge: a render-time throw or a
 * stray rejection never passes through `commands.ts`, and it must still be
 * recorded, still be actionable, and never re-enter itself.
 */

import { beforeEach, describe, expect, it } from "vitest";

import { i18n } from "../i18n";
import { captureGlobalError, installGlobalErrorHandlers } from "./global";
import { clearRecorder, recentFailures } from "./recorder";

beforeEach(() => clearRecorder());

describe("captureGlobalError", () => {
  it("records a Vue render failure with a localised sentence and the raw text in detail", () => {
    captureGlobalError(new Error("cannot read properties of undefined"), "vue:render");
    const failure = recentFailures().at(-1);

    expect(failure?.command).toBe("internal:vue:render");
    expect(failure?.error.code).toBe("unclassified");
    expect(failure?.error.message).toBe(i18n.global.t("diagnostics.internalError"));
    expect(failure?.error.detail).toContain("cannot read properties of undefined");
  });

  it("keeps the source, so the report says which handler fired", () => {
    captureGlobalError("boom", "window.unhandledrejection");
    expect(recentFailures().at(-1)?.error.detail).toContain("window.unhandledrejection");
  });

  it("never throws, whatever it is handed", () => {
    const hostile = { toString: () => { throw new Error("no"); } };
    expect(() => captureGlobalError(hostile, "window.error")).not.toThrow();
    expect(recentFailures()).toHaveLength(1);
  });
});

describe("installGlobalErrorHandlers", () => {
  it("routes a window error event into the same recorder", () => {
    const uninstall = installGlobalErrorHandlers({ config: {} } as never);
    window.dispatchEvent(new ErrorEvent("error", { error: new Error("uncaught"), message: "uncaught" }));
    uninstall();

    expect(recentFailures().map((failure) => failure.command)).toContain("internal:window.error");
  });

  it("records a window error event that carries only a message", () => {
    const uninstall = installGlobalErrorHandlers({ config: {} } as never);
    window.dispatchEvent(new ErrorEvent("error", { message: "script error" }));
    uninstall();
    expect(recentFailures().map((failure) => failure.command)).toContain("internal:window.error");
  });

  it("ignores the browser's ResizeObserver notice", () => {
    // It fires whenever a virtualised list re-measures itself in the same
    // frame. As a danger toast it trains people to ignore the toast the real
    // failures use — and it has no error object to act on.
    const uninstall = installGlobalErrorHandlers({ config: {} } as never);
    window.dispatchEvent(new ErrorEvent("error", { message: "ResizeObserver loop completed with undelivered notifications." }));
    window.dispatchEvent(new ErrorEvent("error", { message: "ResizeObserver loop limit exceeded" }));
    uninstall();

    expect(recentFailures()).toHaveLength(0);
  });

  it("still records a real error that happens to be a ResizeObserver", () => {
    const uninstall = installGlobalErrorHandlers({ config: {} } as never);
    window.dispatchEvent(new ErrorEvent("error", { error: new Error("ResizeObserver loop failed for another reason"), message: "ResizeObserver loop failed for another reason" }));
    uninstall();
    expect(recentFailures()).toHaveLength(1);
  });

  it("removes its listeners on uninstall", () => {
    const uninstall = installGlobalErrorHandlers({ config: {} } as never);
    uninstall();
    clearRecorder();

    // A no-op listener stands in for the uninstalled one, so jsdom does not
    // report the event as an uncaught exception and fail the file.
    const swallow = (): void => {};
    window.addEventListener("error", swallow);
    window.dispatchEvent(new ErrorEvent("error", { error: new Error("after uninstall"), message: "after" }));
    window.removeEventListener("error", swallow);

    expect(recentFailures()).toHaveLength(0);
  });
});
