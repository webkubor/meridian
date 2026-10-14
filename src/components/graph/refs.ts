/**
 * Ref decorations → typed badges.
 *
 * `CommitInfo.decorations` is a list of the strings `git2` decorates a commit
 * with ("HEAD -> main", "origin/main", "v1.2.0", or the fully-qualified
 * "refs/heads/…" forms depending on how the engine collected them). Parsing is
 * deliberately tolerant: the list keeps working whichever spelling arrives.
 *
 * The typed result drives both the canvas badge chips (graph/badges.ts) and the
 * head decoration on the commit bubble.
 */

export type RefKind = "head" | "branch" | "remote" | "tag" | "other";

export interface RefBadge {
  /** Stable id for keying/deduping; unique within one commit's badge list. */
  id: string;
  kind: RefKind;
  /** Short display name: "HEAD", "main", "origin/main", "v1.2.0". */
  label: string;
  /** The full ref name when the input carried one (used for tooltips/commands). */
  ref?: string;
}

export interface ParseDecorationsOptions {
  /** Known remote names; lets a bare "origin/main" be typed as remote. */
  remotes?: readonly string[];
}

const KIND_ORDER: Record<RefKind, number> = { head: 0, branch: 1, remote: 2, tag: 3, other: 4 };

/**
 * Parse the decoration list into badges, deduped and ordered head → branch →
 * remote → tag → other (the order they are drawn in).
 */
export function parseDecorations(
  decorations: readonly string[] | undefined,
  options: ParseDecorationsOptions = {},
): RefBadge[] {
  const badges: RefBadge[] = [];
  const seen = new Set<string>();
  const push = (badge: RefBadge): void => {
    if (seen.has(badge.id)) return;
    seen.add(badge.id);
    badges.push(badge);
  };

  for (const raw of decorations ?? []) {
    const text = raw.trim();
    if (!text) continue;
    if (text === "HEAD") {
      push({ id: "head", kind: "head", label: "HEAD", ref: "HEAD" });
      continue;
    }
    // "HEAD -> main": HEAD currently points at that branch.
    if (text.startsWith("HEAD -> ")) {
      push({ id: "head", kind: "head", label: "HEAD", ref: "HEAD" });
      push(classify(text.slice("HEAD -> ".length), options));
      continue;
    }
    if (text.startsWith("tag: ")) {
      push(classify(`refs/tags/${text.slice("tag: ".length)}`, options));
      continue;
    }
    push(classify(text, options));
  }

  badges.sort((a, b) => KIND_ORDER[a.kind] - KIND_ORDER[b.kind] || a.label.localeCompare(b.label));
  return badges;
}

/** True when the decoration list carries the HEAD marker. */
export function hasHeadDecoration(decorations: readonly string[] | undefined): boolean {
  for (const raw of decorations ?? []) {
    const text = raw.trim();
    if (text === "HEAD" || text.startsWith("HEAD -> ")) return true;
  }
  return false;
}

function classify(name: string, options: ParseDecorationsOptions): RefBadge {
  if (name.startsWith("refs/heads/")) {
    const label = name.slice("refs/heads/".length);
    return { id: `branch:${label}`, kind: "branch", label, ref: name };
  }
  if (name.startsWith("refs/remotes/")) {
    const label = name.slice("refs/remotes/".length);
    return { id: `remote:${label}`, kind: "remote", label, ref: name };
  }
  if (name.startsWith("refs/tags/")) {
    const label = name.slice("refs/tags/".length);
    return { id: `tag:${label}`, kind: "tag", label, ref: name };
  }
  const prefix = name.includes("/") ? name.slice(0, name.indexOf("/")) : "";
  if (prefix && (options.remotes ?? []).includes(prefix)) {
    return { id: `remote:${name}`, kind: "remote", label: name };
  }
  if (name === "HEAD") {
    return { id: "head", kind: "head", label: "HEAD", ref: "HEAD" };
  }
  return { id: `branch:${name}`, kind: "branch", label: name };
}
