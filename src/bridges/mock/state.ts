/**
 * Mutable in-memory store for browser mock mode.
 *
 * Handlers in `handlers.ts` mutate this state and call `emit()` to drive the
 * same store-refresh flows the real backend triggers through
 * `meridian://app-event`. `resetMockState()` restores the seed (used by tests
 * and by the "reset demo" palette action).
 */

import {
  freshBranches,
  freshCommits,
  freshEntries,
  type MockBranch,
  type MockCommit,
  type MockEntry,
} from "./fixtures";

export interface MockRepo {
  id: string;
  gitdir: string;
  workdir: string;
  /** Detached HEAD when null. */
  headBranch: string | null;
  /** Detached HEAD when null. */
  headOid: string | null;
  state: string;
  isEmpty: boolean;
  isBare: boolean;
  discovered: boolean;
}

export interface MockRemote {
  name: string;
  fetchUrl: string;
  pushUrl: string;
  fetchRefspecs: string[];
  pushRefspecs: string[];
  trackedBranches: string[];
}

export interface MockState {
  repos: MockRepo[];
  commits: MockCommit[];
  branches: MockBranch[];
  entries: MockEntry[];
  remotes: MockRemote[];
  staged: Set<string>;
  debugEnabled: boolean;
  updateInstalled: boolean;
  logLines: string[];
  crashNote: {
    at: string;
    message: string;
    location: string | null;
    backtraceExcerpt: string | null;
    kind: "panic" | "recovered_panic" | "unclean_exit";
  } | null;
}

export const MOCK_REPO_ID = "/Users/webkubor/dev/clients/meridian/.git";

function seedState(): MockState {
  const commits = freshCommits();
  const main = commits.find((c) => c.decorations.includes("main"));
  return {
    repos: [
      {
        id: MOCK_REPO_ID,
        gitdir: MOCK_REPO_ID,
        workdir: "/Users/webkubor/dev/clients/meridian",
        headBranch: "main",
        headOid: main?.oid ?? null,
        state: "clean",
        isEmpty: false,
        isBare: false,
        discovered: true,
      },
    ],
    commits,
    branches: freshBranches(commits),
    entries: freshEntries(),
    remotes: [
      {
        name: "origin",
        fetchUrl: "https://github.com/webkubor/meridian.git",
        pushUrl: "https://github.com/webkubor/meridian.git",
        fetchRefspecs: ["+refs/heads/*:refs/remotes/origin/*"],
        pushRefspecs: ["refs/heads/*:refs/heads/*"],
        trackedBranches: ["main"],
      },
    ],
    staged: new Set(
      freshEntries().filter((e) => e.staged).map((e) => e.path),
    ),
    debugEnabled: false,
    updateInstalled: false,
    logLines: [
      "2026-01-30T09:00:00Z INFO  meridian::boot — engine started (mock bridge)",
      "2026-01-30T09:00:00Z INFO  meridian::repo — opened /Users/webkubor/dev/clients/meridian",
      "2026-01-30T09:00:01Z DEBUG meridian::status — 11 entries scanned",
    ],
    crashNote: null,
  };
}

// Lazily seeded: a module-level `seedState()` call would be a top-level side
// effect, which keeps the whole mock graph alive in production bundles even
// when every entry point is behind a `MODE === "mock"` guard.
let state: MockState | null = null;

export function mockState(): MockState {
  if (!state) state = seedState();
  return state;
}

export function resetMockState(): void {
  state = seedState();
}

/** Payload for `refsChanged` / `workingCopyChanged` frames. */
export type MockEvent =
  | { kind: "reposChanged" }
  | { kind: "refsChanged"; payload: { repo: string } }
  | { kind: "workingCopyChanged"; payload: { repo: string } };

type Listener = (event: MockEvent) => void;

const listeners = new Set<Listener>();

/** Programmable event source backing `onAppEvent` in mock mode. */
export function onMockEvent(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function emitMockEvent(event: MockEvent): void {
  for (const listener of [...listeners]) listener(event);
}

/** Resolve an open repo by id, tolerating workdir vs gitdir confusion. */
export function repoById(id: string): MockRepo {
  const current = mockState();
  const repo = current.repos.find((r) => r.id === id || r.workdir === id);
  if (!repo) {
    throw new Error(
      JSON.stringify({
        code: "invalid_repository",
        message: `No open repository for ${id}`,
        domain: "git",
        retryable: false,
      }),
    );
  }
  return repo;
}

/** Throw a wire-shaped error (normalizeError understands this). */
export function mockError(
  code: string,
  message: string,
  domain: string = "git",
): never {
  throw new Error(
    JSON.stringify({ code, message, domain, retryable: false }),
  );
}
