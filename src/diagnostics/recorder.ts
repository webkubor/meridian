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

/**
 * One redaction rule table, shared in substance with `src-tauri/src/logging.rs`.
 *
 * The Rust side guards everything the backend writes (the log file, the crash
 * note, the exported report) and this side guards what the webview records and
 * puts into a copied report or a link. Two rule sets that disagree are two
 * privacy promises, so both run the same fixture
 * (`src-tauri/tests/fixtures/redaction-contract.json`): a rule added here and
 * forgotten there fails a test instead of leaking.
 *
 * Restraint matters as much as coverage. Commit ids, branch names, scp-style
 * remotes and `author=` must survive, or every report becomes useless.
 *
 * `\b` is ASCII-only in JS; the Rust patterns use `(?-u:\b)` so both sides draw
 * word boundaries in the same places.
 */
const RULES: ReadonlyArray<readonly [RegExp, string]> = [
  // A home directory, wherever a path starts: never mid-URL (`example.com/home/docs`).
  [/(^|[\s"'`=(\[,;:<>]|file:\/\/)\/(?:Users|home)\/[^/\s"'`<>:;,)\]]+/g, "$1~"],
  // Userinfo up to the LAST `@` before the first `/`: a password may contain `@`, `?` or `#`.
  [/([a-z][a-z0-9+.-]*:\/\/)[^/\s]+@/gi, "$1[redacted]@"],
  [/(?:gh[pousr]_[A-Za-z0-9_]{10,}|github_pat_[A-Za-z0-9_]{10,}|glpat-[A-Za-z0-9_-]{10,}|xox[baprs]-[A-Za-z0-9-]{8,})/g, "[redacted]"],
  [/\bAKIA[0-9A-Z]{16}\b/g, "[redacted]"],
  // Authorization with any scheme (Bearer, Basic, Token, ...).
  [/(\b(?:proxy-)?authorization["']?\s*[:=]\s*["']?)(?:[a-z][a-z0-9-]*\s+)?[^\s,;"'}]+/gi, "$1[redacted]"],
  [/(\bbearer\s+)[A-Za-z0-9._~+/=-]{16,}/gi, "$1[redacted]"],
  // `key=value` whose key ENDS in a secret word (private_token, client_secret, password, api-key);
  // ending rather than containing keeps `author=` and `authority=` intact.
  [/\b([a-z0-9_.-]*(?:token|password|passwd|secret|api[_-]?key|apikey|auth)\s*=\s*)(?:"[^"]*"|'[^']*'|[^&#\s"']+)/gi, "$1[redacted]"],
];

/** Replace lone surrogates with U+FFFD. A half emoji cannot cross IPC: serde_json rejects the escape and the whole command fails. */
export function wellFormed(text: string): string {
  return text.replace(/([\ud800-\udbff][\udc00-\udfff])|[\ud800-\udfff]/g, (match, pair: string | undefined) => (pair ? match : "\ufffd"));
}

/** `slice(0, max)` that never leaves half of a surrogate pair at the cut. */
export function clip(text: string, max: number): string {
  if (text.length <= max) return text;
  let end = max;
  const last = text.charCodeAt(end - 1);
  if (last >= 0xd800 && last <= 0xdbff) end -= 1;
  return text.slice(0, end);
}

/** The exact `$HOME` becomes `~`, but only where it ends on a path boundary: `/Users/bob` must not turn `/Users/bobby` into `~by`. */
function replaceHome(text: string, home: string): string {
  const base = home.replace(/\/+$/, "");
  if (base === "") return text;
  let out = "";
  let copied = 0;
  for (let at = text.indexOf(base); at !== -1; at = text.indexOf(base, at + base.length)) {
    const next = text.charAt(at + base.length);
    if (next === "" || !/[\p{L}\p{N}_.-]/u.test(next)) {
      out += text.slice(copied, at) + "~";
      copied = at + base.length;
    }
  }
  return out + text.slice(copied);
}

/** Applied before retaining data, and again at copy/export boundaries. `home` is only known to tests; the app relies on the `/Users/<any>` rule. */
export function redactText(input: string, home?: string): string {
  let value = home ? replaceHome(input, home) : input;
  for (const [pattern, replacement] of RULES) value = value.replace(pattern, replacement);
  return wellFormed(value);
}

export function redactPayload(payload: ErrorPayload): ErrorPayload {
  return {
    code: payload.code,
    domain: payload.domain,
    retryable: payload.retryable,
    message: clip(redactText(payload.message), 2048),
    ...(payload.detail === undefined ? {} : { detail: clip(redactText(payload.detail), 8192) }),
    ...(payload.params === undefined ? {} : {
      params: Object.fromEntries(Object.entries(payload.params).map(([key, value]) => [key, clip(redactText(value), 200)])),
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
      : allowed && typeof value === "string" ? clip(redactText(value), 200) : "[redacted]";
  }
  return { argKeys, args: safe };
}
