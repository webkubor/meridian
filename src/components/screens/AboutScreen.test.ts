import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it } from "vitest";

import type { AppInfo, ReleaseNotes, UpdateCheck } from "../../bridges/bindings";
import i18n, { setLocale } from "../../i18n";
import AboutScreen from "./AboutScreen.vue";

const info: AppInfo = {
  name: "Meridian",
  version: "0.1.0",
  commit: "abc123def456",
  buildProfile: "release",
  rustVersion: "1.82",
  osName: "macos",
  osVersion: "14.5",
  arch: "aarch64",
  repoSlug: "example/meridian",
  updatesConfigured: true,
  canSelfUpdate: true,
  diagnostics: "Meridian 0.1.0 (abc123def456)\nmacos 14.5 · aarch64 · rustc 1.82",
};

const notes: ReleaseNotes = {
  markdown: ["## [1.0.0] - 2026-01-01", "", "### Added", "", "- **Things** work."].join("\n"),
  locale: "en",
  currentVersion: "0.1.0",
};

function mountScreen(overrides: Record<string, unknown> = {}) {
  return mount(AboutScreen, {
    props: {
      info: { status: "data", data: info },
      check: { status: "data", data: { kind: "checked", status: { currentVersion: "0.1.0", latestVersion: "0.1.0" } } },
      notes: { status: "data", data: notes },
      install: { phase: "idle", received: 0, total: null, error: null },
      installProgress: null,
      autoCheck: true,
      lastCheckedAt: null,
      locale: "en",
      ...overrides,
    },
    global: { plugins: [i18n] },
  });
}

beforeEach(() => {
  setLocale("en");
});

describe("AboutScreen — what it reports", () => {
  it("shows the version and the short commit", () => {
    const text = mountScreen().text();
    expect(text).toContain("0.1.0");
    expect(text).toContain("abc123def456");
  });

  it("says the build is a release, not a development build", () => {
    expect(mountScreen().text()).toContain("release");
  });

  it("renders the diagnostics text the backend composed, not a rebuilt one", () => {
    // Rebuilding it here would be a second implementation of the same string,
    // and the copy the user pastes into a public issue is the one nobody tests.
    expect(mountScreen().text()).toContain("rustc 1.82");
  });
});

describe("AboutScreen — the update state", () => {
  it("says the running version is the latest when the versions match", () => {
    const text = mountScreen().text();
    expect(text).toContain("is the latest release");
    // Nothing to install, so no install button.
    expect(text).not.toContain("Download and install");
  });

  it("offers the install when a newer version exists", () => {
    const text = mountScreen({
      check: {
        status: "data",
        data: { kind: "checked", status: { currentVersion: "0.1.0", latestVersion: "0.2.0" } },
      },
    }).text();
    expect(text).toContain("0.2.0 is available");
    expect(text).toContain("Download and install");
  });

  it("does not offer an install for a debug build even when a newer version exists", () => {
    const text = mountScreen({
      info: { status: "data", data: { ...info, buildProfile: "debug", canSelfUpdate: false } },
      check: {
        status: "data",
        data: { kind: "checked", status: { currentVersion: "0.1.0", latestVersion: "0.2.0" } },
      },
    }).text();
    // The state is still reported honestly; the button that would replace a
    // working tree under a developer is simply absent.
    expect(text).toContain("0.2.0 is available");
    expect(text).not.toContain("Download and install");
  });

  it("explains why it cannot check when no endpoint is configured", () => {
    const check: UpdateCheck = { kind: "notConfigured" };
    const text = mountScreen({ check: { status: "data", data: check } }).text();
    expect(text).toContain("Updates are not set up in this build");
    // A build that cannot check must not offer a check either.
    expect(text).toContain("Check for updates");
  });

  it("names the development build as the reason it will not self-replace", () => {
    const check: UpdateCheck = { kind: "unsupportedInDebugBuild" };
    const text = mountScreen({ check: { status: "data", data: check } }).text();
    expect(text).toContain("development build");
  });

  it("reports a failed check without implying anything changed", () => {
    const text = mountScreen({
      check: { status: "error", error: { code: "io", message: "timeout", retryable: true, domain: "fs" } },
    }).text();
    expect(text).toContain("Could not check for updates");
    expect(text).toContain("Nothing was changed");
  });

  it("shows the download percentage while installing", () => {
    const text = mountScreen({
      check: {
        status: "data",
        data: { kind: "checked", status: { currentVersion: "0.1.0", latestVersion: "0.2.0" } },
      },
      install: { phase: "downloading", received: 3_000_000, total: 6_000_000, error: null },
      installProgress: 0.5,
    }).text();
    expect(text).toContain("50%");
  });

  it("offers a restart once the install has finished", () => {
    const text = mountScreen({
      check: {
        status: "data",
        data: { kind: "checked", status: { currentVersion: "0.1.0", latestVersion: "0.2.0" } },
      },
      install: { phase: "readyToRestart", received: 6_000_000, total: 6_000_000, error: null },
    }).text();
    expect(text).toContain("restart to finish");
    expect(text).toContain("Restart now");
  });
});

describe("AboutScreen — the language switchers", () => {
  it("renders the option labels from the catalogue, not as raw keys", () => {
    const text = mountScreen().text();
    expect(text).toContain("English");
    expect(text).toContain("简体中文");
    // A missing key renders as the key itself, which is the failure this catches.
    expect(text).not.toContain("language.");
  });

  it("emits the chosen interface locale rather than switching it itself", async () => {
    // The screen is presentational: switching is the shell's job, and a screen
    // that mutated global state could not be rendered in two languages at once.
    //
    // There are two switchers on this screen, and they are not interchangeable:
    // the release-notes one changes which changelog is displayed, the one at the
    // bottom changes the application. Each is addressed by the label of the
    // group it sits in, so a third one cannot be mistaken for either.
    const wrapper = mountScreen();
    const appSwitcher = wrapper
      .findAll(".row")
      .find((row) => row.text().includes("Language"))
      ?.findAll("button")
      .find((button) => button.text() === "简体中文");
    expect(appSwitcher).toBeDefined();

    await appSwitcher?.trigger("click");
    expect(wrapper.emitted("update:locale")?.[0]).toEqual(["zh-CN"]);
    // Not the notes event: reading the notes in Chinese is a different choice.
    expect(wrapper.emitted("notesLocale")).toBeUndefined();
  });

  it("offers the release notes in the other language without changing the interface", async () => {
    const wrapper = mountScreen();
    const notesSwitcher = wrapper
      .findAll(".notes-head button")
      .find((button) => button.text() === "简体中文");
    expect(notesSwitcher).toBeDefined();

    await notesSwitcher?.trigger("click");
    expect(wrapper.emitted("notesLocale")?.[0]).toEqual(["zh-CN"]);
    expect(wrapper.emitted("update:locale")).toBeUndefined();
  });

  it("shows which language the notes on screen are actually in", () => {
    // The toggle's selected side follows the file the backend sent, not the
    // interface language — otherwise it would claim to show Chinese while
    // displaying English.
    const wrapper = mountScreen({
      notes: { status: "data", data: { ...notes, locale: "zh_cn" } },
    });
    const selected = wrapper
      .findAll(".notes-head button")
      .filter((button) => button.attributes("aria-checked") === "true" || button.classes().includes("active"));
    expect(selected.map((button) => button.text())).toEqual(["简体中文"]);
  });
});

describe("AboutScreen — feedback", () => {
  it("emits the kind and the typed title, and opens nothing itself", () => {
    const wrapper = mountScreen();
    const bugButton = wrapper.findAll("button").find((button) => button.text() === "Report a problem");
    expect(bugButton).toBeDefined();
    bugButton?.trigger("click");
    expect(wrapper.emitted("feedback")?.[0]).toEqual(["bug", ""]);
  });

  it("tells the reporter not to paste private material", () => {
    expect(mountScreen().text()).toContain("Do not paste repository contents");
  });
});

describe("AboutScreen — release notes", () => {
  it("renders the changelog it was given, with markup applied", () => {
    const wrapper = mountScreen();
    expect(wrapper.text()).toContain("Things");
    // `**Things**` must become an element, not literal asterisks.
    expect(wrapper.find("strong").text()).toBe("Things");
    expect(wrapper.text()).not.toContain("**");
  });
});

describe("AboutScreen — the diagnostics entry point", () => {
  it("offers a way into the diagnostics screen, and navigates nothing itself", async () => {
    // The screen is presentational: it reports the intent, the shell routes.
    const wrapper = mountScreen();
    const button = wrapper.findAll("button").find((candidate) => candidate.text() === "Open diagnostics");
    expect(button).toBeDefined();

    await button?.trigger("click");
    expect(wrapper.emitted("openDiagnostics")).toHaveLength(1);
  });

  it("still offers the copy-diagnostics action beside it", async () => {
    const wrapper = mountScreen();
    const button = wrapper.findAll("button").find((candidate) => candidate.text() === "Copy diagnostics");
    await button?.trigger("click");
    expect(wrapper.emitted("copyDiagnostics")).toHaveLength(1);
  });
});
