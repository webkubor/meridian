/**
 * The report is the one artefact that leaves the machine — copied, exported, or
 * pasted into a public issue — so it is checked as a redaction boundary rather
 * than a formatting concern.
 */

import { describe, expect, it } from "vitest";

import type { AppInfo, DebugState } from "../bridges/bindings";
import { buildReport, compactReport, type ReportContext } from "./report";
import type { CallRecord, FailureRecord } from "./recorder";

const info: AppInfo = {
  name: "Meridian",
  version: "0.1.0",
  commit: "abc123",
  buildProfile: "release",
  rustVersion: "1.82",
  osName: "macos",
  osVersion: "14.5",
  arch: "aarch64",
  repoSlug: "example/meridian",
  updatesConfigured: true,
  canSelfUpdate: true,
  diagnostics: "Meridian 0.1.0\nrepo: /Users/someone/dev/clients/meridian",
};

const debug: DebugState = { enabled: false, source: "off", logDir: "/Users/someone/Library/Logs", logFile: "meridian.log", level: "info" };

const call: CallRecord = { command: "wc_status_load", startedAt: "2026-01-01T00:00:00.000Z", durationMs: 12, ok: false, errorCode: "io", errorDomain: "fs", argKeys: ["repo"] };

const TOKEN = `ghp_${"A".repeat(24)}`;

const failure: FailureRecord = {
  id: "failure-1",
  at: "2026-01-01T00:00:00.000Z",
  command: "wc_status_load",
  error: { code: "io", message: "Could not read /Users/someone/dev/repo", detail: `remote said: bad credentials for ${TOKEN}`, retryable: true, domain: "fs" },
};

function context(overrides: Partial<ReportContext> = {}): ReportContext {
  return { appInfo: info, failure, calls: [call], failures: [failure], locale: "en", routeName: "diagnostics", debug, ...overrides };
}

describe("buildReport", () => {
  it("names the failure by code, domain, command and time", () => {
    const report = buildReport(context());
    expect(report).toContain('"code": "io"');
    expect(report).toContain('"domain": "fs"');
    expect(report).toContain("wc_status_load");
    expect(report).toContain("2026-01-01T00:00:00.000Z");
    expect(report).toContain("[selected failure]");
  });

  it("redacts every home path it carries", () => {
    const report = buildReport(context());
    expect(report).not.toContain("/Users/someone");
    expect(report).toContain("~");
  });

  it("redacts a credential that reached the detail", () => {
    expect(buildReport(context())).not.toContain(TOKEN);
  });

  it("carries the route NAME, never a repository path", () => {
    expect(buildReport(context())).toContain('"route": "diagnostics"');
  });

  it("says so plainly when no failure is selected", () => {
    expect(buildReport(context({ failure: null }))).toContain("No failure selected");
  });

  it("does not throw when the app information never loaded", () => {
    expect(() => buildReport(context({ appInfo: null }))).not.toThrow();
  });
});

describe("compactReport", () => {
  it("fits inside the cap it advertises", () => {
    const noisy: CallRecord[] = Array.from({ length: 40 }, (_, index) => ({ ...call, command: `command_with_a_long_name_${index}`, args: { repo: "x".repeat(200) } }));
    const compact = compactReport(context({ calls: noisy, failures: [] }));
    expect(compact.length).toBeLessThanOrEqual(2500);
    expect(compact.endsWith("[truncated]")).toBe(true);
  });

  it("returns the whole report when it is short", () => {
    expect(compactReport(context())).toBe(buildReport(context()));
  });
});
