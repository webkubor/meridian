// @vitest-environment node
//
// This file reads the two changelog files off disk. Under jsdom, `new URL(...,
// import.meta.url)` resolves against the dev-server origin rather than the file
// system, so the fixtures come back ENOENT. The rest of the suite stays on
// jsdom (components need `localStorage` and `document`); this one file opts out.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { findEntry, parseChangelog, parseInline, safeHref } from "./changelog";

function repoFile(name: string): string {
  return readFileSync(fileURLToPath(new URL(`../../../${name}`, import.meta.url)), "utf8");
}

describe("parseChangelog — a document", () => {
  const sample = [
    "# Changelog",
    "",
    "Notable changes to Meridian.",
    "",
    "## [Unreleased]",
    "",
    "### Added",
    "",
    "- Something new.",
    "",
    "## [1.2.0] - 2026-01-02",
    "",
    "A short introduction.",
    "",
    "### Fixed",
    "",
    "- A bug.",
    "- Another bug that wraps",
    "  onto a second line.",
    "  - and a nested one.",
    "",
    "### Removed",
    "",
    "- Something gone.",
  ].join("\n");

  it("reads every version heading in order", () => {
    const changelog = parseChangelog(sample);
    expect(changelog.entries.map((entry) => entry.version)).toEqual(["Unreleased", "1.2.0"]);
  });

  it("flags the unreleased section and leaves its date null", () => {
    const [unreleased, released] = parseChangelog(sample).entries;
    expect(unreleased.unreleased).toBe(true);
    expect(unreleased.date).toBeNull();
    expect(released.unreleased).toBe(false);
    expect(released.date).toBe("2026-01-02");
  });

  it("keeps the document title out of the preamble", () => {
    const { preamble } = parseChangelog(sample);
    expect(preamble).toEqual(["Notable changes to Meridian."]);
  });

  it("collects prose between a heading and its groups into the summary", () => {
    const released = findEntry(parseChangelog(sample), "1.2.0");
    expect(released?.summary).toEqual(["A short introduction."]);
  });

  it("groups bullets under their heading", () => {
    const released = findEntry(parseChangelog(sample), "1.2.0");
    expect(released?.groups.map((group) => group.title)).toEqual(["Fixed", "Removed"]);
    expect(released?.groups[1].items.map((item) => item.text)).toEqual(["Something gone."]);
  });

  it("joins a wrapped bullet into one item rather than splitting it", () => {
    const released = findEntry(parseChangelog(sample), "1.2.0");
    const items = released?.groups[0].items ?? [];
    expect(items[1].text).toBe("Another bug that wraps onto a second line.");
  });

  it("records nesting depth instead of flattening it", () => {
    const released = findEntry(parseChangelog(sample), "1.2.0");
    const items = released?.groups[0].items ?? [];
    expect(items.map((item) => item.depth)).toEqual([0, 0, 1]);
  });
});

describe("parseChangelog — a malformed document", () => {
  it("returns no entries rather than throwing", () => {
    expect(parseChangelog("").entries).toEqual([]);
    expect(parseChangelog("just prose, no headings").entries).toEqual([]);
  });

  it("keeps prose that appears under a group with no bullet", () => {
    const changelog = parseChangelog(["## [1.0.0]", "", "### Added", "", "An unbulleted note."].join("\n"));
    // Dropping this would silently lose a line the author wrote.
    expect(changelog.entries[0].groups[0].items.map((item) => item.text)).toEqual([
      "An unbulleted note.",
    ]);
  });

  it("does not treat a link definition as reader-facing text", () => {
    const changelog = parseChangelog(
      ["## [1.0.0]", "", "[1.0.0]: https://example.com/1.0.0"].join("\n"),
    );
    expect(changelog.entries[0].summary).toEqual([]);
    expect(changelog.entries[0].groups).toEqual([]);
  });
});

describe("parseInline", () => {
  it("splits bold, code and links and leaves the rest as text", () => {
    expect(parseInline("**Repositories** — open a `repo`, see [docs](https://x.test).")).toEqual([
      { kind: "strong", text: "Repositories" },
      { kind: "text", text: " — open a " },
      { kind: "code", text: "repo" },
      { kind: "text", text: ", see " },
      { kind: "link", text: "docs", href: "https://x.test" },
      { kind: "text", text: "." },
    ]);
  });

  it("treats an unterminated marker as ordinary text", () => {
    // The reader should see the character the author typed, not nothing.
    expect(parseInline("a **b")).toEqual([{ kind: "text", text: "a **b" }]);
    expect(parseInline("a `b")).toEqual([{ kind: "text", text: "a `b" }]);
  });

  it("returns one text segment for prose with nothing to mark up", () => {
    expect(parseInline("plain")).toEqual([{ kind: "text", text: "plain" }]);
  });
});

describe("safeHref", () => {
  it("allows web and mail links", () => {
    expect(safeHref("https://keepachangelog.com/en/1.1.0/")).toBe(
      "https://keepachangelog.com/en/1.1.0/",
    );
    expect(safeHref("mailto:a@b.test")).toBe("mailto:a@b.test");
  });

  it("refuses schemes that would turn a link into code", () => {
    expect(safeHref("javascript:alert(1)")).toBeNull();
    expect(safeHref("data:text/html,<script>alert(1)</script>")).toBeNull();
    // A packaged app has no origin for a relative link to resolve against.
    expect(safeHref("/settings")).toBeNull();
  });
});

// The two documents that actually ship. A parser that only handles synthetic
// input is not evidence that the Release notes screen works.
describe.each(["CHANGELOG.md", "CHANGELOG.zh-CN.md"])("the shipped %s", (name) => {
  const changelog = parseChangelog(repoFile(name));

  it("parses into version sections", () => {
    expect(changelog.entries.length).toBeGreaterThanOrEqual(2);
  });

  it("has an Unreleased section with no date and a dated release", () => {
    const unreleased = findEntry(changelog, "Unreleased");
    expect(unreleased?.unreleased).toBe(true);
    expect(unreleased?.date).toBeNull();

    const dated = changelog.entries.filter((entry) => !entry.unreleased);
    expect(dated.length).toBeGreaterThanOrEqual(1);
    for (const entry of dated) {
      expect(entry.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    }
  });

  it("gives every group a title and at least one item", () => {
    for (const entry of changelog.entries) {
      for (const group of entry.groups) {
        expect(group.title.trim()).not.toBe("");
        expect(group.items.length).toBeGreaterThan(0);
      }
    }
  });

  it("leaves no stray Markdown heading markers in the item text", () => {
    for (const entry of changelog.entries) {
      for (const group of entry.groups) {
        for (const item of group.items) {
          expect(item.text).not.toMatch(/^#/);
          // A bullet marker left at the start means the regex missed a form.
          expect(item.text).not.toMatch(/^[-*]\s/);
        }
      }
    }
  });
});

describe("the two shipped changelogs agree on which versions exist", () => {
  it("lists the same version labels", () => {
    const en = parseChangelog(repoFile("CHANGELOG.md")).entries.map((entry) => entry.version);
    const zh = parseChangelog(repoFile("CHANGELOG.zh-CN.md")).entries.map((entry) => entry.version);
    // The same guarantee `changelog_parity.rs` makes on the Rust side, asserted
    // here too so a frontend-only change cannot introduce a mismatch unnoticed.
    expect(zh).toEqual(en);
  });
});
