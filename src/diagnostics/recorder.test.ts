/**
 * The recorder's two promises: bounded memory, and nothing private retained.
 *
 * The privacy cases are the reason this file exists. A log that leaks a token
 * once has already leaked it, so the redaction rules are pinned by example
 * rather than trusted to review.
 */

import { beforeEach, describe, expect, it } from "vitest";

import type { ErrorPayload } from "../bridges/bindings";
import {
  clearRecorder,
  recentCalls,
  recentFailures,
  recordCall,
  recordFailure,
  redactText,
  safeArgumentSummary,
  setRecorderDebug,
} from "./recorder";

function payload(overrides: Partial<ErrorPayload> = {}): ErrorPayload {
  return { code: "io", message: "A file operation failed.", retryable: true, domain: "fs", ...overrides };
}

beforeEach(() => {
  clearRecorder();
  setRecorderDebug(false);
});

describe("redaction", () => {
  it("collapses a home directory to ~", () => {
    expect(redactText("/Users/someone/dev/repo/.git/index.lock")).toBe("~/dev/repo/.git/index.lock");
  });

  it("strips credentials from a remote URL", () => {
    const password = `p${"w".repeat(12)}`;
    const out = redactText(`https://alice:${password}@example.com/team/repo.git`);
    expect(out).not.toContain(password);
    expect(out).toContain("[redacted]@example.com");
  });

  // Built at run time rather than written literally: a test fixture that looks
  // like a real credential is one secret-scanner away from being treated as one.
  const tokens: Array<[string, string]> = [
    ["GitHub personal access", `ghp_${"A".repeat(24)}`],
    ["GitHub fine-grained", `github_pat_11ABCDEFG0123456789_abcdefghijklmnop`],
    ["GitLab", `glpat-${"b".repeat(20)}`],
    ["Slack", `xoxb-${"1234567890123"}-abcdefghijklmnop`],
    ["AWS access key", `AKIA${"C".repeat(16)}`],
  ];

  it.each(tokens)("removes a %s credential wherever it appears", (_kind, token) => {
    expect(redactText(`remote rejected ${token} for push`)).not.toContain(token);
  });

  it("removes a bearer token and a credential carried in a query string", () => {
    const bearer = ["abc", "def", "ghi"].join(".");
    const queryToken = `q${"z".repeat(20)}`;
    expect(redactText(`Authorization: Bearer ${bearer}`)).not.toContain(bearer);
    expect(redactText(`https://example.com/callback?token=${queryToken}`)).not.toContain(queryToken);
  });

  it("leaves ordinary git text alone", () => {
    expect(redactText("branch main is behind origin/main by 2 commits")).toBe(
      "branch main is behind origin/main by 2 commits",
    );
  });
});

describe("safeArgumentSummary", () => {
  it("keeps key names and nothing else while debug mode is off", () => {
    const summary = safeArgumentSummary({ args: { repo: "/Users/me/repo", name: "feature/x" } }, "branch_create");
    expect(summary.argKeys).toEqual(["name", "repo"]);
    expect(summary.args).toBeUndefined();
  });

  it("retains allowlisted scalars in debug mode and redacts everything else", () => {
    setRecorderDebug(true);
    const summary = safeArgumentSummary(
      { args: { repo: "/Users/me/repo", name: "feature/x", message: "fix: secret-ish commit body", force: true, paths: ["a", "b"] } },
      "branch_create",
    );
    expect(summary.args).toEqual({
      repo: "[redacted]",
      name: "feature/x",
      message: "[redacted]",
      force: true,
      paths: "[redacted]",
    });
  });

  it("does not read a value it will not keep", () => {
    setRecorderDebug(true);
    let read = 0;
    const args = { args: {} as Record<string, unknown> };
    Object.defineProperty(args.args, "repo", {
      enumerable: true,
      get() {
        read += 1;
        return "/Users/me/repo";
      },
    });
    safeArgumentSummary(args, "wc_status_load");
    expect(read).toBe(0);
  });
});

describe("ring buffers", () => {
  it("keeps the last 300 calls, newest last", () => {
    for (let index = 0; index < 320; index += 1) {
      recordCall({ command: `command_${index}`, startedAt: new Date(index).toISOString(), durationMs: 1, ok: true, argKeys: [] });
    }
    const calls = recentCalls();
    expect(calls).toHaveLength(300);
    expect(calls[0]?.command).toBe("command_20");
    expect(calls[calls.length - 1]?.command).toBe("command_319");
  });

  it("keeps the last 100 failures and redacts them on the way in", () => {
    for (let index = 0; index < 120; index += 1) {
      recordFailure(payload({ detail: `path /Users/me/repo/${index}` }), "wc_status_load", new Date(index));
    }
    const failures = recentFailures();
    expect(failures).toHaveLength(100);
    expect(failures[0]?.error.detail).toBe("path ~/repo/20");
    expect(failures[99]?.error.detail).toBe("path ~/repo/119");
  });

  it("redacts a credential that arrives inside a payload param", () => {
    const password = `s${"e".repeat(14)}`;
    const record = recordFailure(payload({ params: { url: `https://user:${password}@host/repo.git` } }));
    expect(record.error.params?.url).not.toContain(password);
  });
});
