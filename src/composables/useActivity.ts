/**
 * The activity list — what the toolbar's activity chip opens.
 *
 * The Rust queue (`src-tauri/src/queue.rs`) only *serialises* commands per
 * repository; it keeps no record of them, so there was nothing behind the chip
 * to show and the chip was hardcoded to "unavailable". Tracking it here is not
 * a consolation prize: the frontend is the only place that knows the user
 * pressed a button, which is the only thing an activity list is a record of.
 *
 * What this deliberately is not: a progress feed. Nothing here knows how far
 * along a fetch is, so an entry says "running" and "started at", not "42%".
 * Inventing a number would be worse than not having one.
 *
 * Entries are capped and finished ones are dropped from the top once the cap
 * is hit, so a long session does not accumulate a list nobody will read.
 */

import { computed, ref } from "vue";

export type ActivityState = "running" | "done" | "failed";

export interface ActivityEntry {
  id: number;
  /** Catalogue key for the operation's name, or raw text for one-offs. */
  labelKey: string;
  /** Overrides `labelKey` when the entry has a name of its own. */
  label?: string;
  state: ActivityState;
  startedAt: number;
  finishedAt: number | null;
  /** Set on `failed`; already localised by the caller that has the context. */
  error?: string | null;
}

const MAX_ENTRIES = 50;

const entries = ref<ActivityEntry[]>([]);
let nextId = 1;

/** Entries still running, newest first. What the chip counts. */
const running = computed(() => entries.value.filter((entry) => entry.state === "running"));

export function useActivity() {
  /** Records the start of an operation and returns its id. */
  function start(labelKey: string, label?: string): number {
    const id = nextId;
    nextId += 1;
    const entry: ActivityEntry = {
      id,
      labelKey,
      label,
      state: "running",
      startedAt: Date.now(),
      finishedAt: null,
    };
    entries.value = [entry, ...entries.value].slice(0, MAX_ENTRIES);
    return id;
  }

  function finish(id: number, state: Exclude<ActivityState, "running">, error?: string | null): void {
    entries.value = entries.value.map((entry) =>
      entry.id === id
        ? { ...entry, state, finishedAt: Date.now(), error: error ?? null }
        : entry,
    );
  }

  /** Forget everything that is not running — the "clear finished" action. */
  function clearFinished(): void {
    entries.value = running.value;
  }

  return { entries, running, start, finish, clearFinished };
}