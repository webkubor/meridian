/**
 * Synthetic data builders for browser mock mode (`vite --mode mock`).
 *
 * Pure functions only: every builder returns fresh objects so a handler may
 * mutate its copy without leaking into the seed. The shape authority is
 * `src/bridges/bindings.ts`; the status view mapping mirrors
 * `src-tauri/src/domain/status.rs` (one enum, three renderings) exactly.
 */

import type {
  Changeset,
  ChangesetEntry,
  CommitDetail,
  CommitInfo,
  DiffDocument,
  DiffHunk,
  EntryStatus,
  FileStatus,
  GraphLayout,
  RefDecoration,
  Signature,
  StatusDelta,
  StatusSummary,
  UiStatusView,
} from "../bindings";

/* ------------------------------------------------------------------ */
/* Deterministic PRNG — same data on every boot, easier to debug.      */
/* ------------------------------------------------------------------ */

/** mulberry32: tiny, seedable, good enough for fixture jitter. */
export function makeRandom(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** 40 hex digits derived from a counter — stable, unique, plausible. */
function oidFor(n: number): string {
  let out = "";
  let x = (n + 0x9e3779b9) >>> 0;
  for (let i = 0; i < 10; i += 1) {
    x = (Math.imul(x ^ (x >>> 16), 0x45d9f3b) >>> 0) ^ n;
    out += (x >>> (i % 4) * 7).toString(16).padStart(8, "0").slice(0, 4);
  }
  return out.slice(0, 40);
}

/* ------------------------------------------------------------------ */
/* Status view mapping — mirrors src-tauri/src/domain/status.rs.       */
/* ------------------------------------------------------------------ */

const STATUS_VIEWS: Record<FileStatus, UiStatusView> = {
  added: { short: "A", text: "Added", tone: "positive" },
  copied: { short: "C", text: "Copied", tone: "info" },
  deleted: { short: "D", text: "Deleted", tone: "danger" },
  modified: { short: "M", text: "Modified", tone: "warning" },
  renamed: { short: "R", text: "Renamed", tone: "positive" },
  unmerged: { short: "U", text: "Unmerged", tone: "danger" },
  untracked: { short: "?", text: "Untracked", tone: "neutral" },
  ignored: { short: "!", text: "Ignored", tone: "muted" },
};

export function statusView(code: FileStatus): UiStatusView {
  return { ...STATUS_VIEWS[code] };
}

export function summarize(entries: EntryStatus[]): StatusSummary {
  const summary: StatusSummary = { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 };
  for (const entry of entries) {
    if (entry.code === "untracked") summary.untracked += 1;
    else if (entry.code === "unmerged") summary.conflicted += 1;
    else if (entry.staged) summary.staged += 1;
    else summary.unstaged += 1;
  }
  return summary;
}

export function statusDelta(repo: string, entries: EntryStatus[]): StatusDelta {
  return { repo, entries: entries.map((e) => ({ ...e })), summary: summarize(entries) };
}

/* ------------------------------------------------------------------ */
/* People / time.                                                      */
/* ------------------------------------------------------------------ */

const NOW = Math.floor(Date.now() / 1000);
const HOUR = 3600;
const DAY = 24 * HOUR;

export const PEOPLE: Signature[] = [
  { name: "Web Kubor", email: "webkubor@163.com", time: 0 },
  { name: "Ada Lin", email: "ada@example.com", time: 0 },
  { name: "Mori Tanaka", email: "mori@example.com", time: 0 },
  { name: "Jules Verne", email: "jules@example.com", time: 0 },
];

/* ------------------------------------------------------------------ */
/* Commit history — ~140 commits on main + feature branches.           */
/* ------------------------------------------------------------------ */

export interface MockCommit {
  oid: string;
  shortOid: string;
  parents: string[];
  summary: string;
  message: string;
  author: Signature;
  committer: Signature;
  decorations: string[];
  /** Synthetic branch label used for graph lanes ("main" | "feature/…"). */
  lane: string;
}

const MAIN_SUBJECTS = [
  "engine: harden worktree discovery against bare repos",
  "ui: tighten row density in the commit list",
  "engine: cache ref resolution per command batch",
  "shell: remember sidebar width across restarts",
  "engine: stream diff hunks instead of buffering",
  "ui: align branch chips to the baseline grid",
  "bridge: normalize error payloads before capture",
  "engine: guard index writes behind a lock file",
  "docs: document the four list-state contract",
  "ui: keyboard navigation for the history pane",
  "engine: speed up status scans on cold start",
  "shell: polish focus rings for keyboard users",
  "engine: detect rename pairs across directories",
  "bridge: batch event emissions per refresh",
  "ui: empty states for every pane",
  "engine: prune stale merge state on open",
  "tests: cover the IPC contract for remotes",
  "shell: route quick actions through the palette",
  "engine: track assume-unchanged bits faithfully",
  "ui: animated disclosure for hunk groups",
];

const FEATURE_SUBJECTS = [
  "feat: stage-by-hunk interaction",
  "feat: blame annotations for the working copy",
  "feat: signed-commit verification badges",
  "feat: submodule overview pane",
];

function buildHistory(): MockCommit[] {
  const random = makeRandom(20260130);
  const commits: MockCommit[] = [];
  let n = 0;
  let t = NOW - 96 * DAY;
  let mainParent: string | null = null;

  const push = (
    lane: string,
    subject: string,
    parents: string[],
    decorations: string[],
  ): MockCommit => {
    const oid = oidFor(n);
    n += 1;
    t += Math.floor(HOUR * (2 + random() * 20));
    const person = PEOPLE[Math.floor(random() * PEOPLE.length)]!;
    const author: Signature = { ...person, time: t };
    const commit: MockCommit = {
      oid,
      shortOid: oid.slice(0, 7),
      parents: [...parents],
      summary: subject,
      message: `${subject}\n\nBody: ${subject.split(":")[1]?.trim() ?? subject} — synthetic mock history entry.`,
      author,
      committer: { ...author },
      decorations: [...decorations],
      lane,
    };
    commits.push(commit);
    return commit;
  };

  // 130 landed commits on main, with merged feature spurs every ~20 commits.
  for (let i = 0; i < 130; i += 1) {
    const subject = MAIN_SUBJECTS[i % MAIN_SUBJECTS.length]!;
    const parent = mainParent ? [mainParent] : [];
    const decorations: string[] = [];
    if (i === 0) decorations.push("v0.1.0");
    const commit = push("main", subject, parent, decorations);
    mainParent = commit.oid;

    if (i > 0 && i % 20 === 0 && i < 120) {
      // A short-lived branch: 2–3 commits, then merged back into main.
      const featureName = `feature/spur-${i}`;
      let tip = commit.oid;
      const count = 2 + Math.floor(random() * 2);
      for (let j = 0; j < count; j += 1) {
        const c = push(
          featureName,
          FEATURE_SUBJECTS[(i + j) % FEATURE_SUBJECTS.length]!,
          [tip],
          [],
        );
        tip = c.oid;
      }
      const merge = push("main", `merge: ${featureName}`, [mainParent!, tip], []);
      mainParent = merge.oid;
    }
  }

  // feature/brazos: still open, 3 commits ahead of main (never merged).
  let tip = mainParent!;
  for (let j = 0; j < 3; j += 1) {
    const c = push("feature/brazos", FEATURE_SUBJECTS[j % FEATURE_SUBJECTS.length]!, [tip], []);
    tip = c.oid;
  }

  // Origin is 1 behind local main: last local commit not yet pushed.
  const mainHead = commits[commits.length - 1];
  mainHead.decorations.push("main");
  const originTip = mainHead.parents[0]!;
  const originCommit = commits.find((c) => c.oid === originTip);
  originCommit?.decorations.push("origin/main");

  const brazosTip = commits[commits.length - 1];
  if (brazosTip !== mainHead) brazosTip.decorations.push("feature/brazos");

  return commits;
}

// Built fresh on every call: a module-level `buildHistory()` would be a
// top-level side effect that keeps this whole module alive (un-tree-shaken)
// in production bundles even when every entry point is mode-guarded.
export function freshCommits(): MockCommit[] {
  return buildHistory().map((c) => ({
    ...c,
    parents: [...c.parents],
    author: { ...c.author },
    committer: { ...c.committer },
    decorations: [...c.decorations],
  }));
}

export interface MockBranch {
  name: string;
  fullName: string;
  oid: string;
  isRemote: boolean;
  upstream?: string;
  ahead: number;
  behind: number;
}

export function freshBranches(commits: MockCommit[]): MockBranch[] {
  const byDecoration = (needle: string) =>
    commits.find((c) => c.decorations.includes(needle)) ?? commits[commits.length - 1]!;
  const main = byDecoration("main");
  const originMain = byDecoration("origin/main");
  const brazos = commits[commits.length - 1]!;
  return [
    {
      name: "main",
      fullName: "refs/heads/main",
      oid: main.oid,
      isRemote: false,
      upstream: "origin/main",
      ahead: 1,
      behind: 0,
    },
    {
      name: "feature/brazos",
      fullName: "refs/heads/feature/brazos",
      oid: brazos.oid,
      isRemote: false,
      upstream: undefined,
      ahead: 3,
      behind: 0,
    },
    {
      name: "origin/main",
      fullName: "refs/remotes/origin/main",
      oid: originMain.oid,
      isRemote: true,
      ahead: 0,
      behind: 0,
    },
  ];
}

/* ------------------------------------------------------------------ */
/* Working copy — entries covering all eight FileStatus codes.         */
/* ------------------------------------------------------------------ */

export interface MockEntry {
  path: string;
  code: FileStatus;
  staged: boolean;
  assumeUnchanged: boolean;
}

function seedEntries(): MockEntry[] {
  return [
    { path: "src/engine/status.rs", code: "modified", staged: true, assumeUnchanged: false },
    { path: "src/bridges/commands.ts", code: "modified", staged: true, assumeUnchanged: false },
    { path: "src/panes/HistoryPane.vue", code: "modified", staged: false, assumeUnchanged: false },
    { path: "src/features/blame.ts", code: "added", staged: true, assumeUnchanged: false },
    { path: "src/features/blame.spec.ts", code: "untracked", staged: false, assumeUnchanged: false },
    { path: "src/engine/legacy_index.rs", code: "deleted", staged: false, assumeUnchanged: false },
    { path: "src/engine/diff.rs", code: "renamed", staged: true, assumeUnchanged: false },
    { path: "docs/notes/rename-map.txt", code: "copied", staged: false, assumeUnchanged: false },
    { path: "src/engine/merge.rs", code: "unmerged", staged: false, assumeUnchanged: false },
    { path: "coverage/lcov.info", code: "ignored", staged: false, assumeUnchanged: false },
    { path: "src/styles/tokens.css", code: "modified", staged: false, assumeUnchanged: true },
  ];
}

export function freshEntries(): MockEntry[] {
  return seedEntries().map((e) => ({ ...e }));
}

export function toEntryStatuses(entries: MockEntry[]): EntryStatus[] {
  return entries.map((e) => ({
    path: e.path,
    code: e.code,
    staged: e.staged,
    assumeUnchanged: e.assumeUnchanged,
    view: statusView(e.code),
  }));
}

/* ------------------------------------------------------------------ */
/* Diffs.                                                              */
/* ------------------------------------------------------------------ */

function codeHunks(path: string, status: FileStatus): DiffHunk[] {
  const name = path.split("/").pop() ?? path;
  const added = status === "added" || status === "untracked";
  const deleted = status === "deleted";
  const lines: DiffHunk["lines"] = added
    ? [
        { kind: "context", oldLineno: 1, newLineno: 1, text: "use std::collections::BTreeMap;" },
        { kind: "added", newLineno: 2, text: "/// Blame annotations, keyed by line." },
        { kind: "added", newLineno: 3, text: "pub struct Blame {" },
        { kind: "added", newLineno: 4, text: "    lines: BTreeMap<u32, Signature>," },
        { kind: "added", newLineno: 5, text: "}" },
      ]
    : deleted
      ? [
          { kind: "context", oldLineno: 1, newLineno: 1, text: "// legacy index shim" },
          { kind: "removed", oldLineno: 2, text: "pub fn rebuild() -> usize {" },
          { kind: "removed", oldLineno: 3, text: "    0" },
          { kind: "removed", oldLineno: 4, text: "}" },
          { kind: "context", oldLineno: 5, newLineno: 2, text: "// end of file" },
        ]
      : [
          { kind: "context", oldLineno: 12, newLineno: 12, text: "pub fn scan(repo: &Repo) -> Result<Delta> {" },
          { kind: "removed", oldLineno: 13, text: "    let entries = read_all(repo)?;" },
          { kind: "added", newLineno: 13, text: "    let entries = read_all(repo).cached()?" },
          { kind: "added", newLineno: 14, text: "        .filter_ignored(true);" },
          { kind: "context", oldLineno: 14, newLineno: 15, text: "    Ok(Delta::from(entries))" },
          { kind: "context", oldLineno: 15, newLineno: 16, text: "}" },
          { kind: "removed", oldLineno: 40, text: "    assert_eq!(sum, 41);" },
          { kind: "added", newLineno: 41, text: "    assert_eq!(sum, 42);" },
        ];
  return [
    {
      header: `@@ -12,6 +12,7 @@ ${name}`,
      oldStart: 12,
      oldLines: 6,
      newStart: 12,
      newLines: 7,
      lines,
    },
    {
      header: "@@ -101,4 +102,6 @@ tests",
      oldStart: 101,
      oldLines: 4,
      newStart: 102,
      newLines: 6,
      lines: [
        { kind: "context", oldLineno: 101, newLineno: 102, text: "    #[test]" },
        { kind: "added", newLineno: 103, text: "    fn scans_cached_entries() {" },
        { kind: "added", newLineno: 104, text: "        assert!(scan(mock).is_ok());" },
        { kind: "added", newLineno: 105, text: "    }" },
        { kind: "context", oldLineno: 102, newLineno: 106, text: "}" },
        { kind: "noNewline", oldLineno: 106, text: "\\ No newline at end of file" },
      ],
    },
  ];
}

export function diffFor(path: string, code: FileStatus, perspective: string): DiffDocument {
  const binary = /\.(png|jpg|ico)$/.test(path);
  if (binary) {
    return {
      path,
      perspective: perspective as DiffDocument["perspective"],
      change: { status: code, oldSize: 2048, newSize: 2311 },
      isBinary: true,
      hunks: [],
      additions: 0,
      deletions: 0,
      truncated: false,
    };
  }
  const hunks = codeHunks(path, code);
  let additions = 0;
  let deletions = 0;
  for (const hunk of hunks) {
    for (const line of hunk.lines) {
      if (line.kind === "added") additions += 1;
      if (line.kind === "removed") deletions += 1;
    }
  }
  const status = code === "renamed" ? "renamed" : code;
  return {
    path,
    perspective: perspective as DiffDocument["perspective"],
    change: {
      status,
      oldPath: code === "renamed" ? "src/engine/graph.rs" : undefined,
      oldSize: 4812,
      newSize: 5233,
    },
    isBinary: false,
    hunks,
    additions,
    deletions,
    truncated: false,
    syntax: path.endsWith(".rs") ? "rust" : path.endsWith(".ts") ? "typescript" : undefined,
  };
}

/** 1×1 transparent PNG, base64 — enough for the image-diff pane. */
const TINY_PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

export function imageData(path: string, side: "old" | "new") {
  return {
    path,
    side,
    mime: "image/png",
    size: side === "new" ? 2311 : 2048,
    dataBase64: TINY_PNG,
  };
}

/* ------------------------------------------------------------------ */
/* Commit detail / changeset.                                          */
/* ------------------------------------------------------------------ */

export function toCommitInfo(c: MockCommit): CommitInfo {
  return {
    oid: c.oid,
    shortOid: c.shortOid,
    parents: [...c.parents],
    summary: c.summary,
    message: c.message,
    author: { ...c.author },
    committer: { ...c.committer },
    decorations: [...c.decorations],
  };
}

function refDecorations(c: MockCommit): RefDecoration[] {
  return c.decorations.map((name) => ({
    name,
    fullName: name.startsWith("v")
      ? `refs/tags/${name}`
      : name.startsWith("origin/")
        ? `refs/remotes/${name}`
        : `refs/heads/${name}`,
    kind: name.startsWith("v") ? ("tag" as const) : name.startsWith("origin/") ? ("remote" as const) : ("branch" as const),
    isHead: name === "main",
  }));
}

export function commitDetail(c: MockCommit): CommitDetail {
  return {
    oid: c.oid,
    shortOid: c.shortOid,
    parents: [...c.parents],
    parentCount: c.parents.length,
    tree: oidFor(c.oid.length + 777),
    summary: c.summary,
    body: c.message.split("\n").slice(1).join("\n").trim(),
    message: c.message,
    author: { ...c.author },
    committer: { ...c.committer },
    decorations: [...c.decorations],
    refs: refDecorations(c),
    signature: c.author.email.endsWith("163.com") ? "ssh" : "unsigned",
    coauthors:
      c.summary.includes("blame") ? [{ name: "Ada Lin", email: "ada@example.com" }] : [],
    trailers: c.summary.includes("merge") ? [{ token: "Reviewed-by", value: "Ada Lin" }] : [],
    stats: { filesChanged: 3, insertions: 41, deletions: 12 },
  };
}

export function changesetFor(c: MockCommit): Changeset {
  const entries: ChangesetEntry[] = [
    { path: "src/engine/status.rs", status: "modified", binary: false, insertions: 18, deletions: 6 },
    { path: "src/bridges/commands.ts", status: "modified", binary: false, insertions: 21, deletions: 4 },
    { path: "docs/03-bridge.md", status: "modified", binary: false, insertions: 2, deletions: 2 },
  ];
  if (c.parents.length > 1) {
    entries.push({
      path: "src/features/blame.ts",
      oldPath: "src/features/wip.ts",
      status: "renamed",
      binary: false,
      insertions: 30,
      deletions: 0,
    });
  }
  return {
    oid: c.oid,
    parent: c.parents[0] ?? null,
    entries,
    stats: {
      filesChanged: entries.length,
      insertions: entries.reduce((s, e) => s + e.insertions, 0),
      deletions: entries.reduce((s, e) => s + e.deletions, 0),
    },
  };
}

/* ------------------------------------------------------------------ */
/* Graph layout — two lanes (main = 0, feature = 1).                   */
/* ------------------------------------------------------------------ */

export function graphLayout(commits: MockCommit[], oids: string[]): GraphLayout {
  const rowOf = new Map<string, number>();
  oids.forEach((oid, index) => rowOf.set(oid, index));
  const lanes: number[] = oids.map((oid) => {
    const commit = commits.find((c) => c.oid === oid);
    return commit && commit.lane !== "main" ? 1 : 0;
  });
  const segments: GraphLayout["segments"] = [];
  oids.forEach((oid, row) => {
    const commit = commits.find((c) => c.oid === oid);
    if (!commit) return;
    const lane = lanes[row]!;
    const laneIsFeature = commit.lane !== "main";
    if (row + 1 < oids.length) {
      const parentRow = rowOf.get(commit.parents[0] ?? "") ?? -1;
      if (parentRow > row) {
        segments.push({
          fromRow: row,
          toRow: parentRow,
          lane: laneIsFeature ? 1 : 0,
          fromLane: lane,
          toLane: lane,
          kind: laneIsFeature ? "fork" : "straight",
          starting: row === 0,
          ending: false,
        });
      }
    }
    if (commit.parents.length > 1) {
      const second = rowOf.get(commit.parents[1]!) ?? -1;
      if (second >= 0) {
        segments.push({
          fromRow: second,
          toRow: row,
          lane: 1,
          fromLane: 1,
          toLane: 0,
          kind: "merge",
          starting: false,
          ending: true,
        });
      }
    }
  });
  return { lanes, segments, laneCount: 2 };
}

/* ------------------------------------------------------------------ */
/* Canned texts.                                                       */
/* ------------------------------------------------------------------ */

export const RELEASE_NOTES_EN = `## 0.1.0

- Initial public build of the derived engine.
- Status, history, branches, remotes, and integrate panes.
- Contract tests pin every IPC payload.
`;

export const RELEASE_NOTES_ZH = `## 0.1.0

- 派生引擎的首个公开构建。
- 状态、历史、分支、远端与整合面板齐备。
- 每条 IPC 载荷都有契约测试钉住。
`;

export const DIAGNOSTICS_TEXT = `Meridian 0.1.0-mock (browser mock mode)
Build: debug · mock bridge
OS: Browser · UA-resolved
Engine: synthetic fixtures (no libgit2)
Repos: 1 open
`;
