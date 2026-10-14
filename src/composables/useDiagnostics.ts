import { computed, ref } from "vue";

import type { AppInfo, CrashNote, DebugState, ErrorPayload, FeedbackTarget } from "../bridges/bindings";
import {
  appFeedbackCompose,
  diagnosticsDebugGet,
  diagnosticsDebugSet,
  diagnosticsDismissCrash,
  diagnosticsExport,
  diagnosticsLastCrash,
  diagnosticsLogTail,
  diagnosticsOpenDevtools,
  diagnosticsRevealLogs,
} from "../bridges/commands";
import { openExternal } from "../bridges/external";
import { claimFailure } from "../diagnostics/failures";
import { recentCalls, recentFailures, setRecorderDebug, subscribeRecorder, type CallRecord, type FailureRecord } from "../diagnostics/recorder";
import { buildReport } from "../diagnostics/report";
import { i18n, currentLocale } from "../i18n";

export interface DiagnosticsState {
  debug: DebugState | null;
  crash: CrashNote | null;
  calls: readonly CallRecord[];
  failures: readonly FailureRecord[];
  logTail: string;
  busy: boolean;
  error: ErrorPayload | null;
  /** The failure the screen expanded because the route named it. */
  focusId: string | null;
}

export interface DiagnosticsOptions {
  /** The composed backend diagnostics string; read, never rebuilt. */
  appInfo: () => AppInfo | null;
  routeName: () => string;
  /** The failure the route asked to show (`?failure=`), if any. */
  focusId: () => string | null;
  copy: (text: string) => Promise<void>;
  notify: (message: string) => void;
  /** The report URL, offered to the caller instead of a browser that refused it. */
  notifyOpenFailed: (url: string, reason: string) => void;
  relaunch: () => Promise<void>;
}

/** Window-scoped: diagnostics describe the app, not a repository. */
export function useDiagnostics(options: DiagnosticsOptions) {
  const debug = ref<DebugState | null>(null);
  const crash = ref<CrashNote | null>(null);
  const logTail = ref("");
  const busy = ref(false);
  const error = ref<ErrorPayload | null>(null);

  // The buffers live in the recorder module; the screen keeps local refs so
  // Vue re-renders when a command records something, without copying per call.
  const calls = ref<readonly CallRecord[]>(recentCalls());
  const failures = ref<readonly FailureRecord[]>(recentFailures());
  // Kept for the lifetime of the app: the screen reads the module buffers, and
  // a stale copy of them is exactly what a report must not quote.
  subscribeRecorder(() => {
    calls.value = recentCalls();
    failures.value = recentFailures();
  });

  function takeError(caught: unknown): void {
    // The screen shows `error` in its banner and claims it, so the fallback
    // toast does not repeat the banner; a failure in a command the screen owns
    // (refresh, reveal, export) is claimed here.
    error.value = claimFailure(caught, "diagnostics screen");
  }

  async function guard(action: () => Promise<void>): Promise<void> {
    try {
      await action();
    } catch (caught) {
      takeError(caught);
    }
  }

  const state = computed<DiagnosticsState>(() => ({
    debug: debug.value,
    crash: crash.value,
    calls: calls.value,
    failures: failures.value,
    logTail: logTail.value,
    busy: busy.value,
    error: error.value,
    focusId: options.focusId(),
  }));

  const debugEnabled = computed(() => debug.value?.enabled === true);
  const crashKind = computed(() => crash.value?.kind ?? "panic");

  async function refresh(): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      debug.value = await diagnosticsDebugGet();
      setRecorderDebug(debug.value.enabled);
      crash.value = await diagnosticsLastCrash();
      logTail.value = await diagnosticsLogTail(200);
    } catch (caught) {
      takeError(caught);
    } finally {
      busy.value = false;
    }
  }

  async function setDebug(enabled: boolean): Promise<void> {
    await guard(async () => {
      busy.value = true;
      error.value = null;
      try {
        debug.value = await diagnosticsDebugSet(enabled);
        setRecorderDebug(debug.value.enabled);
      } finally {
        busy.value = false;
      }
    });
  }

  const revealLogs = () => guard(async () => { await diagnosticsRevealLogs(); });
  const openDevtools = () => guard(async () => { await diagnosticsOpenDevtools(); });
  const dismissCrash = () => guard(async () => { await diagnosticsDismissCrash(); crash.value = null; });

  function reportContext(failure: FailureRecord | null) {
    return {
      appInfo: options.appInfo(),
      failure,
      calls: calls.value,
      failures: failures.value,
      locale: currentLocale(),
      routeName: options.routeName(),
      debug: debug.value,
      logTail: logTail.value,
    };
  }

  /** The newest failure when the caller names none: a report about something is worth more than one about nothing. */
  const newestFailure = (): FailureRecord | null => failures.value[failures.value.length - 1] ?? null;

  /**
   * The call that produced a failure: the newest call of that command that
   * started no later than the failure. The detail pane shows it, so the
   * duration and the argument keys sit next to the error instead of in a
   * separate table the owner has to correlate by eye.
   */
  function callFor(failure: FailureRecord | null): CallRecord | undefined {
    if (!failure) return undefined;
    return [...calls.value].reverse().find((call) => call.command === failure.command && call.startedAt <= failure.at);
  }

  /** Just this failure: what the owner is looking at, copyable on its own. */
  const failureDetails = (failure: FailureRecord | null): string => {
    if (!failure) return "";
    const call = callFor(failure);
    const lines = [
      `command: ${failure.command}`,
      `at: ${failure.at}`,
      `code: ${failure.error.code}`,
      `domain: ${failure.error.domain}`,
      `message: ${failure.error.message}`,
      failure.error.detail ? `detail: ${failure.error.detail}` : "",
      call ? `duration: ${call.durationMs} ms` : "",
      call ? `arguments: ${call.argKeys.join(", ")}` : "",
    ];
    return lines.filter(Boolean).join("\n");
  };

  const copyFailureDetails = (failure: FailureRecord | null) =>
    guard(async () => { await options.copy(failureDetails(failure ?? newestFailure())); });

  const reportText = (failure: FailureRecord | null = null): string => buildReport(reportContext(failure ?? newestFailure()));

  const copyReport = (failure: FailureRecord | null = null) =>
    guard(async () => { await options.copy(reportText(failure)); });

  /**
   * Writes the whole report to a file in Downloads.
   *
   * The backend adds the app-info block and the log tail it holds, so the text
   * sent here is the part only this side knows, and nothing is cut here: the
   * backend is the one truncator, so the marker a reader sees is the one that
   * says "attach the exported file".
   */
  const exportReport = (failure: FailureRecord | null = null) =>
    guard(async () => {
      const result = await diagnosticsExport(buildReport(reportContext(failure ?? newestFailure()), { app: false, log: false }));
      options.notify(i18n.global.t("diagnostics.actions.exported", { path: result.path }));
    });

  /**
   * Composes the public issue form's URL with `report` attached — and nothing
   * else: no request is made, and the URL is not opened here, because the query
   * string reaches github.com on the first GET and the owner reads it here
   * first (`App.vue` shows a confirmation with exactly this text).
   *
   * Returns null when the build cannot build a link, with the failure recorded.
   */
  async function composeReportLink(report: string): Promise<FeedbackTarget | null> {
    try {
      return await appFeedbackCompose({ kind: "bug", context: report });
    } catch (caught) {
      takeError(caught);
      return null;
    }
  }

  /**
   * Opens a composed link. A browser that refuses it is a result, not a
   * success: `notifyOpenFailed` copies the address and says so, because the
   * button must never do nothing.
   */
  async function openReportLink(target: FeedbackTarget): Promise<boolean> {
    const opened = await openExternal(target.url);
    if (!opened.opened) options.notifyOpenFailed(target.url, opened.reason ?? "");
    return opened.opened;
  }

  return {
    state,
    debugEnabled,
    crashKind,
    refresh,
    setDebug,
    revealLogs,
    dismissCrash,
    openDevtools,
    callFor,
    reportText,
    failureDetails,
    copyFailureDetails,
    copyReport,
    exportReport,
    composeReportLink,
    openReportLink,
  };
}
