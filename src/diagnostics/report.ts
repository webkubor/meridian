import type { AppInfo, CrashNote, DebugState } from "../bridges/bindings";
import { redactText, type FailureRecord, type CallRecord } from "./recorder";

export interface ReportContext {
  appInfo: AppInfo | null;
  /** The failure the report is about; the newest one when the caller names none. */
  failure: FailureRecord | null;
  calls: readonly CallRecord[];
  failures: readonly FailureRecord[];
  locale: string;
  routeName: string;
  debug: DebugState | null;
  /** The tail of the local log, as the Diagnostics screen shows it. */
  logTail?: string;
  /** The previous run's crash note, when there is one. */
  crash?: CrashNote | null;
}

export interface ReportOptions {
  /** The `[app]` block. The backend prepends the same text to an exported file, so the file and the link leave it out. */
  app?: boolean;
  /** The `[log tail]` block. The backend appends its own, longer tail to an exported file. */
  log?: boolean;
}

/** How many of the newest log lines a copied report carries. */
const LOG_LINES = 30;

function safeJson(value: unknown): string {
  return redactText(JSON.stringify(value, null, 2));
}

/**
 * The call that produced a failure: the newest call of that command that
 * started no later than the failure was recorded.
 */
function callFor(failure: FailureRecord, calls: readonly CallRecord[]): CallRecord | undefined {
  return [...calls].reverse().find((call) => call.command === failure.command && call.startedAt <= failure.at);
}

/**
 * The link's content, decoded, for the confirmation before anything is opened.
 *
 * Percent-encoding hides what a reader needs to see, so this decodes each
 * value; a long report is clipped rather than scrolled into unreadability, and
 * the fact that it was clipped is stated rather than implied.
 */
export function describeLink(url: string, maxLines = 60): string {
  const query = url.includes("?") ? url.slice(url.indexOf("?") + 1) : "";
  const lines: string[] = [];
  for (const pair of query.split("&")) {
    if (pair === "") continue;
    const separator = pair.indexOf("=");
    const key = separator === -1 ? pair : pair.slice(0, separator);
    const raw = separator === -1 ? "" : pair.slice(separator + 1);
    let value: string;
    try {
      value = decodeURIComponent(raw.replace(/\+/g, " "));
    } catch {
      value = raw; // A malformed escape is shown as it arrived.
    }
    if (key === "template") continue; // Which form is obvious from the address.
    // The value is usually a multi-line report: keep it line by line, so the
    // preview reads as what it is and the clip counts lines a reader sees.
    const valueLines = value.split("\n");
    lines.push(`${key}: ${valueLines[0] ?? ""}`, ...valueLines.slice(1));
  }
  if (lines.length === 0) return url;
  if (lines.length <= maxLines) return lines.join("\n");
  return `${lines.slice(0, maxLines).join("\n")}\n[… ${lines.length - maxLines} more lines]`;
}

/**
 * The report, most useful part first.
 *
 * The order is deliberate, because the link path hands this to the backend,
 * which cuts it to fit a URL: what survives the cut must be what a maintainer
 * needs. So the failure comes first, then the recent failures and calls
 * newest-first (the oldest entry is the least relevant), and the log last. The
 * previous order put the calls oldest-first ahead of the failures, so the cut
 * landed inside the calls and the failure never reached the report at all.
 */
export function buildReport(context: ReportContext, options: ReportOptions = {}): string {
  const includeApp = options.app ?? true;
  const includeLog = options.log ?? true;
  const selected = context.failure ?? context.failures[context.failures.length - 1] ?? null;

  const sections: string[][] = [["Meridian diagnostic report"]];
  if (includeApp) {
    sections.push([
      "[app]",
      context.appInfo ? redactText(context.appInfo.diagnostics) : "Application information unavailable",
    ]);
  }
  sections.push([
    "[selected failure]",
    selected
      ? safeJson({
          at: selected.at,
          command: selected.command,
          code: selected.error.code,
          domain: selected.error.domain,
          message: selected.error.message,
          detail: selected.error.detail,
          params: selected.error.params,
          call: callFor(selected, context.calls) ?? null,
        })
      : "No failure recorded in this session",
  ]);
  sections.push(["[recent failures, newest first]", safeJson([...context.failures].reverse().slice(0, 10))]);
  sections.push(["[recent calls, newest first]", safeJson([...context.calls].reverse().slice(0, 20))]);
  if (context.crash) {
    // A crash report without the crash is the report the owner submitted last
    // time and could not act on.
    sections.push([
      "[last crash]",
      safeJson({ at: context.crash.at, kind: context.crash.kind, message: context.crash.message, location: context.crash.location }),
    ]);
  }
  sections.push(["[context]", safeJson({ locale: context.locale, route: context.routeName, debug: context.debug })]);
  if (includeLog && context.logTail) {
    sections.push([`[log tail, last ${LOG_LINES} lines]`, context.logTail.split("\n").slice(-LOG_LINES).join("\n")]);
  }
  return redactText(sections.map((section) => section.join("\n")).join("\n\n"));
}
