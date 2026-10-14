import type { ErrorPayload } from "../bridges/bindings";

export interface CallRecord {
  command: string;
  startedAt: string;
  durationMs: number;
  ok: boolean;
  errorCode?: string;
  errorDomain?: string;
  argKeys: string[];
  args?: Record<string, string | number | boolean>;
}

export interface FailureRecord {
  id: string;
  at: string;
  command: string;
  error: ErrorPayload;
}

const CALL_LIMIT = 300;
const FAILURE_LIMIT = 100;
const SCALAR_KEYS = new Set([
  "id", "oid", "branch", "remoteBranch", "localName", "ref", "refname",
  "count", "limit", "skip", "page", "contextLines", "maxDepth", "lines",
  "enabled", "force", "checkout", "track", "bare", "setUpstream", "detectRenames",
  "ignoreWhitespace", "showFull", "firstParent", "includeUntracked", "signOff",
]);
const BRANCH_KEYS = new Set(["name", "old", "new"]);

/** Applied before retaining data, and again at copy/export boundaries. */
export function redactText(input: string): string {
  return input
    .replace(/\/(?:Users|home)\/[^/\s"'<>]+/g, "~")
    .replace(/([a-z][a-z\d+.-]*:\/\/)[^/@\s]+@/gi, "$1[redacted]@")
    .replace(/\b(?:gh[pousr]_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|glpat-[A-Za-z0-9_-]+|xox[baprs]-[A-Za-z0-9-]+)\b/g, "[redacted]")
    .replace(/\bAKIA[0-9A-Z]{16}\b/g, "[redacted]")
    .replace(/(\bauthorization["']?\s*[:=]\s*["']?)(?:(?:bearer|basic)\s+)?[^\s,;"'}]+/gi, "$1[redacted]")
    .replace(/(\bbearer\s+)[A-Za-z0-9._~+/=-]+/gi, "$1[redacted]")
    .replace(/([?&](?:access_token|token|api_key|password)=)[^&#\s]*/gi, "$1[redacted]");
}

export function redactPayload(payload: ErrorPayload): ErrorPayload {
  return {
    code: payload.code,
    domain: payload.domain,
    retryable: payload.retryable,
    message: redactText(payload.message).slice(0, 2048),
    ...(payload.detail === undefined ? {} : { detail: redactText(payload.detail).slice(0, 8192) }),
    ...(payload.params === undefined ? {} : {
      params: Object.fromEntries(Object.entries(payload.params).map(([key, value]) => [key, redactText(value).slice(0, 200)])),
    }),
  };
}

let debugEnabled = false;
export function setRecorderDebug(enabled: boolean): void {
  debugEnabled = enabled;
}

let calls: CallRecord[] = [];
let failures: FailureRecord[] = [];
let sequence = 0;
const listeners = new Set<() => void>();

function publish(): void {
  for (const listener of listeners) {
    try { listener(); } catch { /* Diagnostics observers must not affect a command's outcome. */ }
  }
}

export function recordCall(record: CallRecord): void {
  calls = [...calls, record].slice(-CALL_LIMIT);
  publish();
}

export function recordFailure(error: ErrorPayload, command = "unknown", at = new Date()): FailureRecord {
  const record: FailureRecord = {
    id: `failure-${(sequence += 1)}`,
    at: at.toISOString(),
    command: redactText(command),
    error: redactPayload(error),
  };
  failures = [...failures, record].slice(-FAILURE_LIMIT);
  publish();
  return record;
}

export function recentCalls(): readonly CallRecord[] { return calls; }
export function recentFailures(): readonly FailureRecord[] { return failures; }
export function subscribeRecorder(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export function clearRecorder(): void {
  calls = [];
  failures = [];
  publish();
}

export function safeArgumentSummary(args: unknown, command = ""): Pick<CallRecord, "argKeys" | "args"> {
  const root = args && typeof args === "object" ? args as Record<string, unknown> : {};
  const values = root.args && typeof root.args === "object" ? root.args as Record<string, unknown> : root;
  const argKeys = Object.keys(values).sort();
  if (!debugEnabled) return { argKeys };

  const safe: Record<string, string | number | boolean> = {};
  for (const key of argKeys) {
    // Never inspect unknown values: a Channel, a patch, or a getter is not diagnostic data.
    const descriptor = Object.getOwnPropertyDescriptor(values, key);
    const value: unknown = descriptor?.value;
    const allowed = SCALAR_KEYS.has(key) || (command.startsWith("branch_") && BRANCH_KEYS.has(key));
    safe[key] = allowed && (typeof value === "boolean" || typeof value === "number" && Number.isFinite(value))
      ? value
      : allowed && typeof value === "string" ? redactText(value).slice(0, 200) : "[redacted]";
  }
  return { argKeys, args: safe };
}
