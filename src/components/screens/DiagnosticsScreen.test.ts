import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it } from "vitest";

import type { CrashNote, DebugState } from "../../bridges/bindings";
import i18n, { setLocale } from "../../i18n";
import type { CallRecord, FailureRecord } from "../../diagnostics/recorder";
import DiagnosticsScreen from "./DiagnosticsScreen.vue";

const debug: DebugState = { enabled: true, source: "setting", logDir: "~/Library/Logs/Meridian", logFile: "meridian.log", level: "debug", devtoolsReady: true };

const crash: CrashNote = { at: "2026-01-01T00:00:00Z", message: "index out of bounds", location: "history.rs:412", backtraceExcerpt: "0: meridian::history", kind: "panic" };

const call: CallRecord = { command: "wc_status_load", startedAt: "2026-01-01T00:00:00Z", durationMs: 12, ok: false, errorCode: "io", errorDomain: "fs", argKeys: ["repo"] };

const failure: FailureRecord = {
  id: "failure-1",
  at: "2026-01-01T00:00:00Z",
  command: "wc_status_load",
  error: { code: "io", message: "A file operation failed.", detail: "index.lock", retryable: true, domain: "fs" },
};

function mountScreen(overrides: Record<string, unknown> = {}) {
  return mount(DiagnosticsScreen, {
    props: { debug, crash: null, calls: [], failures: [], logTail: "", busy: false, error: null, focusId: null, ...overrides },
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

  it("names the mode source in words, not as the raw enum", () => {
    const wrapper = mountScreen({ debug: { ...debug, source: "env" } });
    expect(wrapper.text()).not.toContain("source: env");
    expect(wrapper.text()).toContain("MERIDIAN_DEBUG environment variable");
  });
});

describe("DiagnosticsScreen — developer tools", () => {
  it("opens them when this run's window can", () => {
    const wrapper = mountScreen();
    const button = wrapper.findAll("button").find((candidate) => candidate.text().trim() === "Open developer tools");
    expect(button?.attributes("disabled")).toBeUndefined();
  });

  it("offers a restart instead of a button that can only refuse", () => {
    // Debug mode was switched on in this session: the webview decides
    // inspector support at window creation, so the backend always refuses with
    // "restart required" until the next launch.
    const wrapper = mountScreen({ debug: { ...debug, devtoolsReady: false } });
    const button = wrapper.findAll("button").find((candidate) => candidate.text().trim() === "Open developer tools");
    expect(button?.attributes("disabled")).toBeDefined();
    const restart = wrapper.findAll("button").find((candidate) => candidate.text().trim() === "Restart to enable");
    expect(restart).toBeDefined();
    expect(wrapper.text()).toContain("need a restart");
  });

  it("says nothing about developer tools while debug mode is off", () => {
    const off = mountScreen({ debug: { ...debug, enabled: false, source: "off", devtoolsReady: false } });
    expect(off.text()).not.toContain("Open developer tools");
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

  it("labels a recovered panic differently from a run that ended with it", () => {
    // A panic the app caught and carried on from is a bug worth reporting, but
    // it is not "the previous run ended unexpectedly".
    const recovered = mountScreen({ crash: { ...crash, kind: "recovered_panic" } });
    expect(recovered.text()).toContain("recovered internal error");
    expect(recovered.text()).not.toContain("The previous run ended unexpectedly");

    const unclean = mountScreen({ crash: { ...crash, kind: "unclean_exit" } });
    expect(unclean.text()).toContain("did not finish cleanly");
  });

  it("says nothing about a crash when there was none", () => {
    expect(mountScreen().text()).not.toContain("ended unexpectedly");
  });
});

describe("DiagnosticsScreen — failures and calls", () => {
  it("lists a failure with its translated sentence and offers report and full-copy per item", async () => {
    const wrapper = mountScreen({ failures: [failure] });
    expect(wrapper.text()).toContain("A file operation failed.");

    await wrapper.find(".failure-toggle").trigger("click");
    expect(wrapper.text()).toContain("index.lock");

    const report = wrapper.findAll("button").find((button) => button.text() === "Report this problem");
    await report?.trigger("click");
    expect(wrapper.emitted("reportProblem")?.[0]).toEqual([failure]);

    const copy = wrapper.findAll("button").find((button) => button.text() === "Copy full report");
    await copy?.trigger("click");
    expect(wrapper.emitted("copyReport")?.[0]).toEqual([failure]);
  });

  it("expands the failure the route named", () => {
    const wrapper = mountScreen({ failures: [failure], focusId: "failure-1" });
    expect(wrapper.find(".failure-toggle").attributes("aria-expanded")).toBe("true");
    expect(wrapper.text()).toContain("index.lock");
    expect(wrapper.text()).toContain("Focused from the error you were shown");
  });

  it("translates a remote failure instead of printing the backend's English", () => {
    const rejected: FailureRecord = {
      ...failure,
      command: "remote_publish_branch",
      error: { code: "push_rejected", message: "The remote rejected 'main': non-fast-forward", retryable: false, domain: "git", params: { name: "main", class: "publish_rejected" } },
    };
    const wrapper = mountScreen({ failures: [rejected] });
    expect(wrapper.text()).toContain("The remote refused to publish “main”.");
    expect(wrapper.text()).not.toContain("The remote rejected 'main': non-fast-forward");
  });

  it("tabulates the recent commands in local time", () => {
    const wrapper = mountScreen({ calls: [call] });
    expect(wrapper.text()).toContain("wc_status_load");
    expect(wrapper.text()).toContain("Failed (io)");
    expect(wrapper.text()).not.toContain("2026-01-01T00:00:00Z");
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

  it("is a section inside the shell's main, with keyboard-reachable scroll areas", () => {
    const wrapper = mountScreen({ calls: [call], logTail: "12:00:01 INFO started" });
    // A `<main>` nested inside the shell's `<main>` is two landmarks for one
    // page; and a scroll region nobody can focus cannot be scrolled by keyboard.
    expect(wrapper.findAll("main")).toHaveLength(0);
    expect(wrapper.find(".table-scroll").attributes("tabindex")).toBe("0");
    expect(wrapper.find(".table-scroll").attributes("aria-label")).toBeTruthy();
    expect(wrapper.find(".log-tail").attributes("tabindex")).toBe("0");
  });

  it("asks for a refresh as soon as it opens, not only at launch", () => {
    // The tail read at launch predates the failure the owner came to look at.
    expect(mountScreen().emitted("refresh")).toBeTruthy();
  });
});
