/**
 * Two promises about the web layer, pinned by reading the tree rather than by
 * review: it may not reach the network on its own, and the window it runs in
 * may not be allowed to either.
 *
 * The first is the no-telemetry promise. The updater's single request is made
 * from Rust (`app_update_check`), where the disclosure text describes it, so
 * anything network-shaped in `src/` is a promise that has drifted.
 *
 * The second is the enforcement: with `csp: null` nothing stops the webview
 * from making a request, and a bundle that quietly works today is one refactor
 * away from quietly reporting somewhere else.
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

function sourceFiles(directory: string, found: string[] = []): string[] {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) {
      if (entry === "node_modules") continue;
      sourceFiles(path, found);
    } else if (/\.(ts|vue)$/.test(entry) && !/\.test\.ts$/.test(entry)) {
      found.push(path);
    }
  }
  return found;
}

describe("the web layer", () => {
  const files = sourceFiles(resolve(process.cwd(), "src"));

  it("finds the app's own sources", () => {
    expect(files.length).toBeGreaterThan(30);
  });

  it("never reaches the network itself", () => {
    const offenders: string[] = [];
    for (const file of files) {
      const text = readFileSync(file, "utf8");
      // `fetch(` inside a comment or a test name would be a false positive, so
      // the report names the file for a human to look at.
      // `git.fetch()` is a store verb, not the network: only a bare call counts.
      for (const pattern of [/(^|[^.\w])fetch\s*\(/, /new\s+XMLHttpRequest/, /new\s+WebSocket/, /navigator\.sendBeacon/, /EventSource/]) {
        if (pattern.test(text)) offenders.push(`${file.replace(process.cwd(), "")}: ${pattern}`);
      }
    }
    expect(offenders, "src/ must not talk to the network").toEqual([]);
  });

  it("calls the backend only through the instrumented bridge", () => {
    const callers = files.filter((file) => /from\s+"@tauri-apps\/api\//.test(readFileSync(file, "utf8")));
    const relative = callers.map((file) => file.replace(resolve(process.cwd(), "src"), "src"));
    expect(relative.sort()).toEqual([
      "src/bridges/commands.ts",
      "src/bridges/events.ts",
      // The event bus: the only other Tauri surface, and it carries no invoke.
      "src/composables/useAppEvents.ts",
    ]);
  });
});

describe("the window's policy", () => {
  /** `directive: values`, split per directive so one directive cannot excuse another. */
  function directives(): Record<string, string> {
    const parts = (config.app.security.csp ?? "").split(";").map((part) => part.trim()).filter(Boolean);
    const parsed: Record<string, string> = {};
    for (const part of parts) {
      const [name, ...values] = part.split(/\s+/);
      parsed[name ?? ""] = values.join(" ");
    }
    return {
      scriptSrc: parsed["script-src"],
      connectSrc: parsed["connect-src"],
      objectSrc: parsed["object-src"],
      frameAncestors: parsed["frame-ancestors"],
      baseUri: parsed["base-uri"],
      formAction: parsed["form-action"],
    };
  }

  const config = JSON.parse(
    readFileSync(resolve(process.cwd(), "src-tauri/tauri.conf.json"), "utf8").replace(/^\s*\/\/.*$/gm, ""),
  ) as { app: { security: { csp: string | null; devCsp?: string | null } } };

  it("is not left open", () => {
    const csp = config.app.security.csp;
    expect(csp, "app.security.csp must not be null").toBeTruthy();
    expect(csp).toContain("default-src 'self'");
    // Tauri needs these for the IPC bridge; without them the app cannot start.
    expect(csp).toContain("ipc:");
    expect(csp).toContain("http://ipc.localhost");
  });

  it("does not allow remote scripts, objects or framing", () => {
    expect(directives().scriptSrc).toBe("'self'");
    expect(directives().objectSrc).toBe("'none'");
    expect(directives().frameAncestors).toBe("'none'");
    expect(directives().baseUri).toBe("'self'");
    expect(directives().formAction).toBe("'none'");
  });

  it("connects to nothing but the IPC bridge", () => {
    expect(directives().connectSrc?.split(/\s+/).sort()).toEqual(["http://ipc.localhost", "ipc:"]);
  });
});
