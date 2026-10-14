/**
 * Interaction completeness.
 *
 * Every `@click` here is a promise to the user: press it, something happens.
 * The cheapest promise to break is the one nobody checks — a component emits
 * `browse`, its parent never binds `@browse`, and the button is drawn, looks
 * enabled, and does absolutely nothing.
 *
 * Nothing in a browser, a screenshot, or a manual pass catches that. The
 * button renders. The layout is right. The only symptom is the one users
 * report as "a lot of things don't respond when I click them", which is
 * indistinguishable from an app that is merely busy.
 *
 * So it gets a test, the same way `handler_completeness.rs` gets one for the
 * Rust side: the drift is caught here rather than by somebody clicking
 * through the app and being annoyed.
 *
 * The allowlist below is not a list of known-broken things — it is a list of
 * *deliberate* ones, each with the reason it is allowed to be unbound. An
 * entry added to silence a failure without fixing it is a regression that now
 * has a comment defending it.
 */

// @vitest-environment node
// This walks the source tree with `fs`; jsdom would only get in the way.

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const SRC = fileURLToPath(new URL(".", import.meta.url));

function vueFiles(dir = SRC): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...vueFiles(path));
    else if (entry.name.endsWith(".vue")) out.push(path);
  }
  return out;
}

/** camelCase and kebab-case are the same event; Vue matches either. */
function kebab(name: string): string {
  return name.replace(/(?<!^)(?=[A-Z])/g, "-").toLowerCase();
}

/**
 * One `<button …>` opening tag, quotes respected.
 *
 * A regex over `[^>]*` stops at the `>` inside `v-if="a && b > c"` and reports
 * a truncated tag that looks like it has no handler. Walking the quotes is
 * four lines and removes a whole class of false positives.
 */
function buttonTags(source: string): string[] {
  const tags: string[] = [];
  const open = /<button\b/g;
  for (let m = open.exec(source); m; m = open.exec(source)) {
    let i = m.index + "<button".length;
    let quote: string | null = null;
    for (; i < source.length; i += 1) {
      const ch = source[i];
      if (quote) {
        if (ch === quote) quote = null;
      } else if (ch === '"' || ch === "'") quote = ch;
      else if (ch === ">") break;
    }
    tags.push(source.slice(m.index, i + 1));
  }
  return tags;
}

const files = vueFiles();
const sources = new Map(files.map((path) => [path, readFileSync(path, "utf8")]));

/**
 * Events a component is allowed to emit without any parent listening.
 *
 * Keyed by `<file>:<event>` so an allowlist entry is always about one named
 * control — a wildcard here would let a genuinely dead button hide behind it.
 */
const ALLOWED_UNBOUND: Record<string, string> = {
  // Presentational only. Every screen that wants the button renders its own
  // EmptyState with an `@action`; nothing relies on the secondary one yet.
  "EmptyState.vue:secondaryAction":
    "Unused affordance. No caller passes a secondary action today; the prop exists for the one that will.",

  // `<VirtualList>` reports three things nothing consumes today: no list pages
  // through it (the commit list has its own windowing and its own
  // `need-more`), and no sibling canvas is drawn against its scroll position —
  // the graph is rendered inside the list's own scroller, not beside it. Kept
  // because a file list will eventually want paging, and wiring them to
  // behaviour that does not exist would be worse than leaving them idle.
  "VirtualList.vue:endReached": "No list pages through VirtualList yet.",
  "VirtualList.vue:scroll": "No sibling surface tracks VirtualList's scroll today.",
  "VirtualList.vue:viewport": "No sibling surface tracks VirtualList's viewport today.",

  // The accounts screen is handed `unavailable` by the shell, because no
  // hosting-account backend exists. That disables "Add account" and leaves the
  // list empty, so the selected-account buttons — and therefore these emits —
  // cannot be reached. They are listed rather than deleted so the day the
  // backend lands, this test says which four handlers still need a parent.
  "AccountScreen.vue:addAccount":
    "Unreachable: the button is disabled while `unavailable` is set.",
  "AccountScreen.vue:connect": "Unreachable: requires a selected account.",
  "AccountScreen.vue:disconnect": "Unreachable: requires a selected account.",
  "AccountScreen.vue:copyKey": "Unreachable: requires a selected account.",
};

/**
 * Emits no caller listens to, each with the reason it is allowed to exist.
 *
 * `<VirtualList>`'s `scroll` and `viewport` are here because its documentation
 * promises a parent can keep a canvas in lockstep with it — which needs a
 * `defineExpose`, not an allowlist entry. Until one exists, nothing can read
 * them, so the entry below records the gap rather than hiding it.
 */
const REF_CONSUMED = new Set<string>();

describe("every emitted event is listened for", () => {
  const bound = new Set<string>();
  for (const source of sources.values()) {
    for (const name of source.matchAll(/@([\w:.-]+)/g)) bound.add(name[1]);
  }

  const unbound: string[] = [];
  for (const [path, source] of sources) {
    const file = path.slice(path.lastIndexOf("/") + 1);
    for (const [, event] of source.matchAll(/emit\(\s*["']([\w:.-]+)["']/g)) {
      if (REF_CONSUMED.has(`${file}:${event}`)) continue;
      if (ALLOWED_UNBOUND[`${file}:${event}`]) continue;
      if (bound.has(event) || bound.has(kebab(event))) continue;
      unbound.push(`${file}:${event}`);
    }
  }

  it("finds none", () => {
    expect(unbound).toEqual([]);
  });
});

describe("every button in a template is wired to something", () => {
  /**
   * A `<button>` with no handler is either a bug or a submit control. Both are
   * decidable from the tag alone, which is why this is a source scan and not
   * a mount-and-inspect: mounting cannot tell "no listener" from "a listener
   * that ignores the event".
   */
  const inert: string[] = [];

  for (const [path, source] of sources) {
    const file = path.slice(path.lastIndexOf("/") + 1);
    for (const tag of buttonTags(source)) {
      if (/@(click|mousedown|keyup|keydown)\b/.test(tag)) continue;
      // A submit button hands its click to the enclosing <form>.
      if (/type=["']submit["']/.test(tag)) continue;
      // The rest are rendered through another component (SplitAction's trigger
      // and WorkingCopyFileList's bulk toggle delegate to a sibling control).
      inert.push(`${file}: ${tag.replace(/\s+/g, " ").slice(0, 80)}`);
    }
  }

  it("finds none", () => {
    expect(inert).toEqual([]);
  });
});