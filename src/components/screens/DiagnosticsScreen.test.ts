import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it } from "vitest";

import type { CrashNote, DebugState } from "../../bridges/bindings";
import i18n, { setLocale } from "../../i18n";
import type { CallRecord, FailureRecord } from "../../diagnostics/recorder";
import DiagnosticsScreen from "./DiagnosticsScreen.vue";

const debug: DebugState = { enabled: true, source: "setting", logDir: "~/Library/Logs/Meridian", logFile: "meridian.log", level: "debug" };

const crash: CrashNote = { at: "2026-01-01T00:00:00Z", message: "index out of bounds", location: "history.rs:412", backtraceExcerpt: "0: meridian::history" };

const call: CallRecord = { command: "wc_status_load", startedAt: "2026-01-01T00:00:00Z", durationMs: 12, ok: false, errorCode: "io", errorDomain: "fs", argKeys: ["repo"] };

const failure: FailureRecord = {
  id: "failure-1",
  at: "2026-01-01T00:00:00Z",
  command: "wc_status_load",
  error: { code: "io", message: "A file operation failed.", detail: "index.lock", retryable: true, domain: "fs" },
};

function mountScreen(overrides: Record<string, unknown> = {}) {
  return mount(DiagnosticsScreen, {
    props: { debug, crash: null, calls: [], failures: [], logTail: "", busy: false, error: null, ...overrides },
    global: { plugins: [i18n] },
  });
}

beforeEach(() => setLocale("en"));

describe("DiagnosticsScreen — the debug switch", () => {
  it("reports the state and emits the requested change", async () => {
    const wrapper = mountScreen();
    const toggle = wrapper.find('button[role="switch"]');
    expect(toggle.attributes("aria-checked")).toBe("true");

    await toggle.trigger("click");
    expect(wrapper.emitted("setDebug")?.[0]).toEqual([false]);
  });

  it("disables the switch and explains why when the environment owns it", () => {
    // MERIDIAN_DEBUG=1 cannot be turned off from the UI, so the control must not
    // pretend it can: disabled plus a sentence, not a switch that snaps back.
    const wrapper = mountScreen({ debug: { ...debug, enabled: true, source: "env" } });
    expect(wrapper.find('button[role="switch"]').attributes("disabled")).toBeDefined();
    expect(wrapper.text()).toContain("MERIDIAN_DEBUG");
  });
});

describe("DiagnosticsScreen — the crash banner", () => {
  it("shows the last crash and emits a dismissal", async () => {
    const wrapper = mountScreen({ crash });
    expect(wrapper.text()).toContain("index out of bounds");
    expect(wrapper.text()).toContain("history.rs:412");

    const dismiss = wrapper.findAll("button").find((button) => button.text() === "Dismiss");
    await dismiss?.trigger("click");
    expect(wrapper.emitted("dismissCrash")).toBeTruthy();
  });

  it("says nothing about a crash when there was none", () => {
    expect(mountScreen().text()).not.toContain("ended unexpectedly");
  });
});

describe("DiagnosticsScreen — failures and calls", () => {
  it("lists a failure with its code and offers copy and report per item", async () => {
    const wrapper = mountScreen({ failures: [failure] });
    expect(wrapper.text()).toContain("A file operation failed.");

    await wrapper.find(".failure-toggle").trigger("click");
    expect(wrapper.text()).toContain("index.lock");

    const report = wrapper.findAll("button").find((button) => button.text() === "Report this problem");
    await report?.trigger("click");
    expect(wrapper.emitted("reportProblem")?.[0]).toEqual([failure]);
  });

  it("tabulates the recent commands", () => {
    const wrapper = mountScreen({ calls: [call] });
    expect(wrapper.text()).toContain("wc_status_load");
    expect(wrapper.text()).toContain("Failed (io)");
  });

  it("states the empty cases rather than rendering a blank card", () => {
    const text = mountScreen().text();
    expect(text).toContain("No failures recorded.");
    expect(text).toContain("No commands recorded.");
    expect(text).toContain("No log entries are available.");
  });

  it("renders the log tail it was given", () => {
    expect(mountScreen({ logTail: "12:00:01 INFO started" }).text()).toContain("12:00:01 INFO started");
  });
});

describe("DiagnosticsScreen — the actions", () => {
  it("emits the report, export and devtools intents", async () => {
    const wrapper = mountScreen();
    const click = async (label: string) => {
      const button = wrapper.findAll("button").find((candidate) => candidate.text() === label);
      await button?.trigger("click");
    };

    await click("Copy diagnostic report");
    await click("Export diagnostics file");
    await click("Open developer tools");

    expect(wrapper.emitted("copyReport")?.[0]).toEqual([null]);
    expect(wrapper.emitted("exportReport")?.[0]).toEqual([null]);
    expect(wrapper.emitted("openDevtools")).toBeTruthy();
  });

  it("offers developer tools only while debug mode is on", () => {
    const off = mountScreen({ debug: { ...debug, enabled: false, source: "off" } });
    expect(off.text()).not.toContain("Open developer tools");
  });
});
