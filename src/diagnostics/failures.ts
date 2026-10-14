import { toRaw } from "vue";
import type { ErrorCode, ErrorDomain, ErrorPayload } from "../bridges/bindings";
import { recordFailure, type FailureRecord } from "./recorder";

const DOMAINS = new Set(["git", "service", "fs", "internal", "cancelled"]);

/** Unknown bridge rejections keep their text in detail, never masquerade as a wire payload. */
export function normalizeError(value: unknown): ErrorPayload {
  try {
    if (value && typeof value === "object") {
      const candidate = value as Partial<ErrorPayload>;
      if (typeof candidate.code === "string" && typeof candidate.message === "string" && DOMAINS.has(candidate.domain ?? "")) {
        return {
          code: candidate.code as ErrorCode,
          message: candidate.message,
          detail: typeof candidate.detail === "string" ? candidate.detail : undefined,
          params: candidate.params && typeof candidate.params === "object"
            ? Object.fromEntries(Object.entries(candidate.params).filter((entry) => typeof entry[1] === "string")) : undefined,
          retryable: candidate.retryable === true,
          domain: candidate.domain as ErrorDomain,
        };
      }
    }
    return { code: "unclassified", message: "An internal error occurred.", detail: value instanceof Error ? value.stack ?? value.message : String(value), retryable: false, domain: "internal" };
  } catch {
    return { code: "unclassified", message: "An internal error occurred.", detail: "Unreadable error", retryable: false, domain: "internal" };
  }
}

interface Pending {
  record: FailureRecord;
  timer?: unknown;
}
export interface FailureNetOptions {
  graceMs?: number;
  now?: () => number;
  schedule?: (callback: () => void, delay: number) => unknown;
  cancel?: (handle: unknown) => void;
  showToast?: (payload: ErrorPayload) => void;
  log?: (record: FailureRecord) => void;
}

/** A factory keeps clocks and timers deterministic in tests; the window uses one instance. */
export function createFailureNet(options: FailureNetOptions = {}) {
  let config = options;
  let objects = new WeakMap<object, FailureRecord>();
  const primitives = new Map<unknown, FailureRecord>();
  const pending = new Map<unknown, Pending>();
  const messages = new Map<string, number>();
  let shownAt: number[] = [];
  const now = () => config.now?.() ?? Date.now();
  const cancel = (handle: unknown) => config.cancel ? config.cancel(handle) : clearTimeout(handle as ReturnType<typeof setTimeout>);
  const keyFor = (value: unknown): unknown => value !== null && typeof value === "object" ? toRaw(value) : value;
  const isObject = (value: unknown): value is object => value !== null && (typeof value === "object" || typeof value === "function");

  function show(key: unknown): void {
    const item = pending.get(key);
    if (!item || !config.showToast) return;
    pending.delete(key);
    const time = now();
    for (const [text, at] of messages) if (time - at >= 2000) messages.delete(text);
    shownAt = shownAt.filter((at) => time - at < 10_000);
    const message = item.record.error.message;
    if (messages.has(message) || shownAt.length >= 5) return;
    messages.set(message, time);
    shownAt.push(time);
    try { config.showToast(item.record.error); } catch { /* Never throw from the last failure handler. */ }
  }

  function markHandled(value: unknown): void {
    const key = keyFor(value);
    const item = pending.get(key);
    if (item) cancel(item.timer);
    pending.delete(key);
  }

  function capture(value: unknown, command = "internal", opts: { handled?: boolean; newCall?: boolean; payload?: ErrorPayload } = {}): FailureRecord {
    const key = keyFor(value);
    const existing = isObject(key) ? objects.get(key) : primitives.get(key);
    if (existing && !opts.newCall) {
      if (opts.handled) markHandled(key);
      return existing;
    }
    const record = recordFailure(opts.payload ?? normalizeError(value), command, new Date(now()));
    if (isObject(key)) objects.set(key, record);
    else {
      primitives.set(key, record);
      if (primitives.size > 100) primitives.delete(primitives.keys().next().value);
    }
    if (!command.startsWith("diagnostics_")) {
      try { config.log?.(record); } catch { /* The primary error remains visible if the log is unavailable. */ }
    }
    markHandled(key);
    if (!opts.handled) {
      const item: Pending = { record };
      pending.set(key, item);
      const schedule = config.schedule ?? ((callback, delay) => setTimeout(callback, delay));
      item.timer = schedule(() => show(key), config.graceMs ?? 250);
    }
    return record;
  }

  function configure(next: FailureNetOptions): void {
    config = { ...config, ...next };
    // Failures raised during bootstrap remain pending until the shell can display them.
    if (next.showToast) for (const [key, item] of pending) {
      if (now() - Date.parse(item.record.at) >= (config.graceMs ?? 250)) show(key);
    }
  }

  function reset(): void {
    for (const item of pending.values()) cancel(item.timer);
    pending.clear();
    primitives.clear();
    objects = new WeakMap();
    messages.clear();
    shownAt = [];
    config = options;
  }

  return { capture, markHandled, configure, reset };
}

const failures = createFailureNet();
export const configureFailureNet = failures.configure;
export const markHandled = failures.markHandled;
export const captureFailure = failures.capture;
export const resetFailureNet = failures.reset;
