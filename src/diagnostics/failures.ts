/**
 * The failure net: every failure is recorded and logged exactly once, and is
 * shown to the user unless something on screen has claimed it.
 *
 * Two promises, and the whole design follows from them:
 *
 *  1. **Never silent.** A failure that nobody renders produces a fallback toast
 *     after a short grace period. "Rendered" is decided by the *renderer*
 *     (`useFailureClaim`), not by the code that caught the error, because the
 *     catch block cannot know whether the pane that would show it is mounted:
 *     a failed history load on the working-copy tab has no history pane to
 *     show it, and used to be marked handled all the same.
 *  2. **Never lost.** An exception thrown by the app's own code inside the same
 *     `try` as a command (a view-model mapper, an upsert) has no pending record
 *     from the bridge. `reportFailure` creates one, so it is logged and shown
 *     like any other failure instead of appearing as raw English in a pane.
 */

import { toRaw } from "vue";
import type { ErrorCode, ErrorDomain, ErrorPayload } from "../bridges/bindings";
import { recordFailure, type FailureRecord } from "./recorder";

export type { FailureRecord };

const DOMAINS = new Set<string>(["git", "service", "fs", "internal", "cancelled"]);
const DETAIL_LIMIT = 4096;

/** The English text carried by a failure that arrived without a wire payload. It is a log line: the UI renders `errorText`, never this. */
const UNCLASSIFIED_MESSAGE = "An internal error occurred.";

function isStringMap(value: unknown): value is Record<string, string> {
  return value !== null && typeof value === "object" && Object.values(value).every((entry) => typeof entry === "string");
}

/** The exact wire shape. A payload that matches is returned as the same object, so identity survives for matching. */
function isWirePayload(value: object): value is ErrorPayload {
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.code === "string" &&
    typeof candidate.message === "string" &&
    typeof candidate.domain === "string" &&
    DOMAINS.has(candidate.domain) &&
    typeof candidate.retryable === "boolean" &&
    (candidate.detail === undefined || typeof candidate.detail === "string") &&
    (candidate.params === undefined || isStringMap(candidate.params))
  );
}

/** Text for anything that is not a wire payload. Never throws; objects keep their content instead of becoming `[object Object]`. */
function describe(value: unknown): string {
  try {
    if (value instanceof Error) return (value.stack ?? value.message).slice(0, DETAIL_LIMIT);
    if (typeof value === "string") return value.slice(0, DETAIL_LIMIT);
    if (value !== null && typeof value === "object") {
      const seen = new WeakSet<object>();
      const json = JSON.stringify(value, (_key, entry: unknown) => {
        if (typeof entry === "bigint") return entry.toString();
        if (entry !== null && typeof entry === "object") {
          if (seen.has(entry)) return "[circular]";
          seen.add(entry);
        }
        return entry;
      });
      if (json !== undefined) return json.slice(0, DETAIL_LIMIT);
    }
    return String(value).slice(0, DETAIL_LIMIT);
  } catch {
    return "Unreadable error";
  }
}

/**
 * Turn whatever was thrown into something every renderer can show.
 *
 * Tauri rejects with the command's own `ErrorPayload` for failures the backend
 * classified, but with a bare string for argument-deserialisation and unknown-
 * command errors, and plugins reject with their own objects. All of them must
 * come out as an `ErrorPayload`: storing the raw value is what left the launch
 * alert empty and the list error saying "Something went wrong." with no code.
 */
export function normalizeError(value: unknown): ErrorPayload {
  try {
    const raw = value !== null && typeof value === "object" ? toRaw(value) : value;
    if (raw !== null && typeof raw === "object" && !(raw instanceof Error)) {
      if (isWirePayload(raw)) return raw;
      const candidate = raw as Record<string, unknown>;
      // Shaped like a payload from a newer backend (an error domain this build
      // does not know): keep the code and the message, and file it as internal.
      if (typeof candidate.code === "string" && typeof candidate.message === "string") {
        return {
          code: candidate.code as ErrorCode,
          message: candidate.message,
          detail: typeof candidate.detail === "string" ? candidate.detail : undefined,
          params: isStringMap(candidate.params) ? candidate.params : undefined,
          retryable: candidate.retryable === true,
          domain: typeof candidate.domain === "string" && DOMAINS.has(candidate.domain) ? (candidate.domain as ErrorDomain) : "internal",
        };
      }
    }
    // A JavaScript error keeps its own words: "Cannot read properties of
    // undefined" is the most actionable line in a report, and the UI shows it
    // as the reason under the localised sentence.
    return {
      code: "unclassified",
      message: raw instanceof Error ? raw.message : UNCLASSIFIED_MESSAGE,
      detail: describe(raw),
      retryable: false,
      domain: "internal",
    };
  } catch {
    return { code: "unclassified", message: UNCLASSIFIED_MESSAGE, detail: "Unreadable error", retryable: false, domain: "internal" };
  }
}

interface Entry {
  record: FailureRecord;
  payload: ErrorPayload;
  createdAt: number;
  pending: boolean;
  timer?: unknown;
}

export interface FailureNetOptions {
  graceMs?: number;
  now?: () => number;
  schedule?: (callback: () => void, delay: number) => unknown;
  cancel?: (handle: unknown) => void;
  /** Shows the fallback toast for a failure nobody claimed. */
  showToast?: (payload: ErrorPayload, record: FailureRecord) => void;
  /** Called once per burst with how many failures the cap kept off screen. */
  showSummary?: (count: number) => void;
  /** Writes a failure to the local log; never called for `diagnostics_*` commands. */
  log?: (record: FailureRecord) => void;
}

/** Two seconds: the same failure shown twice in a row is one failure. */
const DEDUPE_MS = 2_000;
const BURST_WINDOW_MS = 10_000;
const BURST_LIMIT = 5;
/** How long after the cap trips before the "N more" toast is raised, so the count can settle. */
const SUMMARY_DELAY_MS = 1_000;
/** A repeated primitive (`"boom"`) is the same failure only within this window; after it, it is a new one. */
const PRIMITIVE_WINDOW_MS = 1_000;

/** A factory keeps clocks and timers deterministic in tests; the window uses one instance. */
export function createFailureNet(options: FailureNetOptions = {}) {
  let config = options;
  let objects = new WeakMap<object, Entry>();
  const primitives = new Map<unknown, Entry>();
  const pending = new Set<Entry>();
  const recent = new Map<string, number>();
  let shownAt: number[] = [];
  let suppressed = 0;
  let summaryTimer: unknown;

  const now = () => config.now?.() ?? Date.now();
  const schedule = (callback: () => void, delay: number) =>
    config.schedule ? config.schedule(callback, delay) : setTimeout(callback, delay);
  const cancel = (handle: unknown) =>
    config.cancel ? config.cancel(handle) : clearTimeout(handle as ReturnType<typeof setTimeout>);
  const keyFor = (value: unknown): unknown => (value !== null && typeof value === "object" ? toRaw(value) : value);
  const isObject = (value: unknown): value is object => value !== null && (typeof value === "object" || typeof value === "function");

  function lookup(key: unknown): Entry | undefined {
    if (isObject(key)) return objects.get(key);
    const entry = primitives.get(key);
    if (entry && now() - entry.createdAt >= PRIMITIVE_WINDOW_MS) {
      primitives.delete(key);
      return undefined;
    }
    return entry;
  }

  function link(key: unknown, entry: Entry): void {
    if (isObject(key)) {
      objects.set(key, entry);
      return;
    }
    primitives.set(key, entry);
    if (primitives.size > 100) primitives.delete(primitives.keys().next().value);
  }

  function settle(entry: Entry): void {
    if (!entry.pending) return;
    if (entry.timer !== undefined) cancel(entry.timer);
    entry.pending = false;
    pending.delete(entry);
  }

  function flushSummary(): void {
    summaryTimer = undefined;
    const count = suppressed;
    suppressed = 0;
    if (count === 0) return;
    try { config.showSummary?.(count); } catch { /* Never throw from the last failure handler. */ }
  }

  function show(entry: Entry): void {
    if (!entry.pending || !config.showToast) return;
    settle(entry);
    const time = now();
    for (const [key, at] of recent) if (time - at >= DEDUPE_MS) recent.delete(key);
    shownAt = shownAt.filter((at) => time - at < BURST_WINDOW_MS);
    // Code, command and detail are part of the key: two different failures that
    // happen to share a sentence ("An internal error occurred.") are two
    // failures, and a repeated one is one.
    const key = `${entry.payload.code}|${entry.record.command}|${entry.payload.message}|${(entry.payload.detail ?? "").slice(0, 80)}`;
    if (recent.has(key)) return;
    if (shownAt.length >= BURST_LIMIT) {
      // The record and the log already have it; the user is told how many were
      // kept off screen instead of the burst silently ending.
      suppressed += 1;
      summaryTimer ??= schedule(flushSummary, SUMMARY_DELAY_MS);
      return;
    }
    recent.set(key, time);
    shownAt.push(time);
    try { config.showToast(entry.payload, entry.record); } catch { /* Never throw from the last failure handler. */ }
  }

  function markHandled(value: unknown): void {
    const entry = lookup(keyFor(value));
    if (entry) settle(entry);
  }

  /** The id of the record a value belongs to, so a screen can send the user to it. */
  function recordIdFor(value: unknown): string | undefined {
    return lookup(keyFor(value))?.record.id;
  }

  function capture(
    value: unknown,
    command = "internal",
    opts: { handled?: boolean; newCall?: boolean; payload?: ErrorPayload } = {},
  ): FailureRecord {
    const key = keyFor(value);
    const existing = lookup(key);
    if (existing && !opts.newCall) {
      // Known failure seen again (a store re-throwing what the bridge already
      // captured). Let the normalised copy find its entry too, then settle.
      if (opts.payload) link(opts.payload, existing);
      if (opts.handled) settle(existing);
      return existing.record;
    }
    // A second call that rejects with the very same object replaces the first
    // entry: only the latest one can still be claimed.
    if (existing) settle(existing);

    const payload = opts.payload ?? normalizeError(value);
    const record = recordFailure(payload, command, new Date(now()));
    const entry: Entry = { record, payload, createdAt: now(), pending: false };
    link(key, entry);
    link(payload, entry);
    if (!command.startsWith("diagnostics_")) {
      try { config.log?.(record); } catch { /* The primary error remains visible if the log is unavailable. */ }
    }
    if (!opts.handled) {
      entry.pending = true;
      pending.add(entry);
      entry.timer = schedule(() => show(entry), config.graceMs ?? 250);
    }
    return record;
  }

  function configure(next: FailureNetOptions): void {
    config = { ...config, ...next };
    // Failures raised during bootstrap remain pending until the shell can display them.
    if (next.showToast) {
      for (const entry of [...pending]) {
        if (now() - Date.parse(entry.record.at) >= (config.graceMs ?? 250)) show(entry);
      }
    }
  }

  function reset(): void {
    for (const entry of pending) if (entry.timer !== undefined) cancel(entry.timer);
    if (summaryTimer !== undefined) cancel(summaryTimer);
    summaryTimer = undefined;
    suppressed = 0;
    pending.clear();
    primitives.clear();
    objects = new WeakMap();
    recent.clear();
    shownAt = [];
    config = options;
  }

  return { capture, markHandled, recordIdFor, configure, reset };
}

const failures = createFailureNet();
export const configureFailureNet = failures.configure;
export const markHandled = failures.markHandled;
export const captureFailure = failures.capture;
export const resetFailureNet = failures.reset;
export const failureIdFor = failures.recordIdFor;

/**
 * Record a failure and return the payload a renderer can show.
 *
 * Use this wherever a failure is caught and kept for display: the value that
 * comes back is always a well-formed `ErrorPayload` (never a raw string or an
 * `Error`), and the failure is logged and listed in Diagnostics even when it
 * came from the app's own code and never touched the bridge. It is *not* marked
 * as handled: the component that renders the payload claims it
 * (`useFailureClaim`), so a failure whose pane is not on screen still reaches
 * the user as a toast.
 */
export function reportFailure(error: unknown, source: string): ErrorPayload {
  const payload = normalizeError(error);
  failures.capture(error, source, { payload });
  return payload;
}

/**
 * `reportFailure` for the code that shows the failure itself in the same
 * breath (a toast, a dialogue): recorded and logged, but never a second toast.
 */
export function claimFailure(error: unknown, source: string): ErrorPayload {
  const payload = normalizeError(error);
  failures.capture(error, source, { payload, handled: true });
  return payload;
}
