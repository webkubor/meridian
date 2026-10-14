/**
 * The guard in front of the app bundle.
 *
 * A module that never runs leaves the owner with a window and no message, and
 * the failure handlers in `main.ts` are installed after the imports — too late
 * for a syntax error in the graph. This is the only surface that exists at that
 * moment, so it has to keep the panel, show the reason and offer it for copying.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeEach, describe, expect, it } from "vitest";

const source = readFileSync(resolve(process.cwd(), "public/boot-guard.js"), "utf8");

describe("the boot guard", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    delete (window as unknown as { __meridianReady?: unknown }).__meridianReady;
  });

  it("is a plain script, so it runs even when the module graph does not", () => {
    expect(source).not.toContain("import ");
    expect(source).not.toContain("=>");
    expect(source).toContain('window.addEventListener("error"');
  });

  it("shows the first error on the static panel and offers it to the log", () => {
    document.body.innerHTML = `
      <div id="boot">
        <p id="boot-message"></p>
        <button id="boot-copy" type="button">复制 / Copy</button>
      </div>`;
    const invokeCalls: unknown[][] = [];
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = { invoke: (...args: unknown[]) => invokeCalls.push(args) };
    const swallow = (event: Event): void => event.preventDefault();
    window.addEventListener("error", swallow);
    // eslint-disable-next-line no-new-func
    new Function(source)();
    window.removeEventListener("error", swallow);

    const panel = document.getElementById("boot") as HTMLElement;
    expect(panel.hidden).toBe(false);
    window.dispatchEvent(Object.assign(new Event("error"), { message: "SyntaxError: unexpected token" }));
    expect((document.getElementById("boot-message") as HTMLElement).textContent).toContain("SyntaxError");
    // And it is in the local log, so the owner can find it later.
    expect(JSON.stringify(invokeCalls)).toContain("diagnostics_log_write");
    expect(JSON.stringify(invokeCalls)).toContain("SyntaxError");

    // The app takes over once it mounts.
    (window as unknown as { __meridianReady: () => void }).__meridianReady();
    expect(panel.hidden).toBe(true);
  });
});
