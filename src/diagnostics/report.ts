import type { AppInfo, DebugState } from "../bridges/bindings";
import { redactText, type FailureRecord, type CallRecord } from "./recorder";

export interface ReportContext {
  appInfo: AppInfo | null;
  failure: FailureRecord | null;
  calls: readonly CallRecord[];
  failures: readonly FailureRecord[];
  locale: string;
  routeName: string;
  debug: DebugState | null;
}

function safeJson(value: unknown): string {
  return redactText(JSON.stringify(value, null, 2));
}

export function buildReport(context: ReportContext): string {
  const selected = context.failure;
  const selectedCall = selected
    ? [...context.calls].reverse().find((call) => call.command === selected.command && call.startedAt <= selected.at)
    : undefined;
  const lines = [
    "Meridian diagnostic report",
    "",
    "[app]",
    context.appInfo ? redactText(context.appInfo.diagnostics) : "Application information unavailable",
    "",
    "[context]",
    safeJson({ locale: context.locale, route: context.routeName, debug: context.debug }),
    "",
    "[selected failure]",
    selected
      ? safeJson({
          at: selected.at,
          command: selected.command,
          code: selected.error.code,
          domain: selected.error.domain,
          message: selected.error.message,
          detail: selected.error.detail,
          call: selectedCall ?? null,
        })
      : "No failure selected",
    "",
    "[last 20 calls]",
    safeJson(context.calls.slice(-20)),
    "",
    "[last 10 failures]",
    safeJson(context.failures.slice(-10)),
  ];
  return redactText(lines.join("\n"));
}

export function compactReport(context: ReportContext, limit = 2500): string {
  const report = buildReport(context);
  if (report.length <= limit) return report;
  const suffix = "\n[truncated]";
  return `${report.slice(0, Math.max(0, limit - suffix.length))}${suffix}`;
}
