/**
 * `useApp` — the window-scoped state, and the two promises it makes about the
 * network: the update check is the only request, and turning it off has to
 * stick. A silent failure to remember that choice means the next launch asks
 * GitHub again, which is exactly what the disclosure says it will not do.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => ({ kind: "notConfigured" })),
  Channel: class {},
}));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

/** A webview whose storage refuses both reads and writes. */
function localStorageThatFails(): void {
  Object.defineProperty(window, "localStorage", {
    configurable: true,
    value: {
      getItem: () => {
        throw new DOMException("The operation is insecure.", "SecurityError");
      },
      setItem: () => {
        throw new DOMException("Quota exceeded.", "QuotaExceededError");
      },
      removeItem: () => undefined,
      clear: () => undefined,
    },
  });
}

beforeEach(async () => {
  vi.resetModules();
  invokeMock.mockResolvedValue({ kind: "notConfigured" } as never);
  Object.defineProperty(window, "localStorage", { configurable: true, value: window.localStorage });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useApp", () => {
  it("starts with storage that refuses reads", async () => {
    // The i18n runtime guards the same call; an unguarded read at module scope
    // is a blank window with no message.
    localStorageThatFails();
    const { useApp } = await import("./useApp");
    const app = useApp();
    // Default on: the disclosure says the check happens once after launch.
    expect(app.autoCheck.value).toBe(true);
    expect(app.info.value.status).toBe("loading");
  });

  it("still starts when the whole module graph is re-evaluated", async () => {
    localStorageThatFails();
    const first = await import("./useApp");
    const second = await import("./useApp");
    expect(first.useApp().autoCheck.value).toBe(true);
    expect(second.useApp().autoCheck.value).toBe(true);
  });

  it("reports a preference it could not save", async () => {
    const { useApp } = await import("./useApp");
    // Imported in the same batch: `resetModules` gives a fresh recorder to the
    // composable, and a statically imported one would be a different instance.
    const { recentFailures } = await import("../diagnostics/recorder");
    const app = useApp();

    Object.defineProperty(window, "localStorage", {
      configurable: true,
      value: { ...window.localStorage, setItem: () => { throw new DOMException("no", "QuotaExceededError"); } },
    });
    app.setAutoCheck(false);

    // The switch is off in this session …
    expect(app.autoCheck.value).toBe(false);
    // … and the owner is told it will not be remembered, rather than the next
    // launch contacting GitHub as if nothing happened.
    expect(recentFailures().some((failure) => failure.command === "settings.autoCheck")).toBe(true);
  });
});
