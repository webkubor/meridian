/**
 * The report is the one artefact that leaves the machine — copied, exported, or
 * pasted into a public issue — so it is checked as a redaction boundary, as an
 * ordering guarantee (what survives the backend's cut), and as the text the
 * confirmation shows before anything is opened.
 */

import { describe, expect, it } from "vitest";

import type { AppInfo, DebugState } from "../bridges/bindings";
import { buildReport, describeLink, type ReportContext } from "./report";
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

const debug: DebugState = { enabled: false, source: "off", logDir: "/Users/someone/Library/Logs", logFile: "meridian.log", level: "info", devtoolsReady: false };

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

  it("puts the failure first, so it survives the link's cut", () => {
    const older: FailureRecord = { ...failure, id: "failure-0", at: "2025-12-31T00:00:00.000Z", error: { ...failure.error, message: "the oldest failure" } };
    const report = buildReport(context({ failure, failures: [older, failure] }));
    // The failure the reader came for is before any call history.
    expect(report.indexOf("[selected failure]")).toBeLessThan(report.indexOf("[recent calls"));
    expect(report.indexOf("[recent failures")).toBeLessThan(report.indexOf("[recent calls"));
    // Newest first inside the lists.
    expect(report.indexOf('"io"')).toBeLessThan(report.lastIndexOf("Could not read"));
    // And the failure the caller named wins over the newest one.
    const named = buildReport(context({ failure: older, failures: [older, failure] }));
    expect(named).toContain("the oldest failure");
  });

  it("falls back to the newest failure when the caller names none", () => {
    const older: FailureRecord = { ...failure, id: "failure-0", at: "2025-12-31T00:00:00.000Z", error: { ...failure.error, message: "the oldest failure" } };
    const report = buildReport(context({ failure: null, failures: [older, failure] }));
    const selected = report.slice(report.indexOf("[selected failure]"), report.indexOf("[recent failures"));
    expect(selected).toContain("Could not read /Users/someone/dev/repo".replace("/Users/someone", "~"));
    expect(selected).not.toContain("the oldest failure");
  });

  it("says so plainly when nothing failed at all", () => {
    expect(buildReport(context({ failure: null, failures: [] }))).toContain("No failure recorded");
  });

  it("leaves the app block and the log out when the backend adds both", () => {
    const report = buildReport(context({ logTail: "12:00:01 INFO started" }), { app: false, log: false });
    expect(report).not.toContain("[app]");
    expect(report).not.toContain("12:00:01");
    expect(report).toContain("[selected failure]");
  });

  it("carries the last log lines into a copied report", () => {
    const report = buildReport(context({ logTail: "12:00:01 INFO started" }));
    expect(report).toContain("[log tail, last 30 lines]");
    expect(report).toContain("12:00:01 INFO started");
  });

  it("does not throw when the app information never loaded", () => {
    expect(() => buildReport(context({ appInfo: null }))).not.toThrow();
  });
});

describe("describeLink", () => {
  const url = `https://github.com/example/meridian/issues/new?template=bug_report.yml&version=0.1.0+%28abc123%29&diagnostics=${encodeURIComponent("[selected failure]\ncode: io\n")}`;

  it("shows the decoded contents, so the review is of the real text", () => {
    const preview = describeLink(url);
    expect(preview).toContain("version: 0.1.0 (abc123)");
    expect(preview).toContain("[selected failure]");
    // The form name is obvious from the address; the content is not.
    expect(preview).not.toContain("template=");
  });

  it("clips a long report and says how much was left out", () => {
    const long = `https://github.com/o/r/issues/new?diagnostics=${encodeURIComponent(Array.from({ length: 200 }, (_, index) => `line ${index}`).join("\n"))}`;
    const preview = describeLink(long, 10);
    expect(preview).toContain("line 9");
    expect(preview).not.toContain("line 10");
    expect(preview).toContain("190 more lines");
  });
});
