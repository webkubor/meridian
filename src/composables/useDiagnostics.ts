import { computed, onBeforeUnmount, onMounted, ref } from "vue";

import type { AppInfo, CrashNote, DebugState, ErrorPayload } from "../bridges/bindings";
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
import { markHandled, normalizeError } from "../diagnostics/failures";
import { recentCalls, recentFailures, setRecorderDebug, subscribeRecorder, type CallRecord, type FailureRecord } from "../diagnostics/recorder";
import { buildReport, compactReport } from "../diagnostics/report";
import { currentLocale } from "../i18n";

export interface DiagnosticsState {
  debug: DebugState | null;
  crash: CrashNote | null;
  calls: readonly CallRecord[];
  failures: readonly FailureRecord[];
  logTail: string;
  busy: boolean;
  error: ErrorPayload | null;
}

export interface DiagnosticsOptions {
  /** The composed backend diagnostics string; read, never rebuilt. */
  appInfo: () => AppInfo | null;
  routeName: () => string;
  copy: (text: string) => Promise<void>;
  notify: (message: string) => void;
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
  const unsubscribe = subscribeRecorder(() => {
    calls.value = recentCalls();
    failures.value = recentFailures();
  });

  function takeError(caught: unknown): void {
    markHandled(caught);
    error.value = normalizeError(caught);
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
  }));

  const debugEnabled = computed(() => debug.value?.enabled === true);

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
    };
  }

  const reportText = (failure: FailureRecord | null = null): string => buildReport(reportContext(failure));

  const copyReport = (failure: FailureRecord | null = null) =>
    guard(async () => { await options.copy(reportText(failure)); });

  const exportReport = (failure: FailureRecord | null = null) =>
    guard(async () => {
      const result = await diagnosticsExport(compactReport(reportContext(failure)));
      options.notify(result.path);
    });

  /**
   * Opens the public issue form with the compact report attached.
   *
   * Nothing is transmitted: the backend builds a URL, the user's browser opens
   * it, and the user reads and submits it themselves (docs/02 §16).
   */
  const reportProblem = (failure: FailureRecord | null = null) =>
    guard(async () => {
      const target = await appFeedbackCompose({ kind: "bug", context: compactReport(reportContext(failure)) });
      await openExternal(target.url);
    });

  onMounted(() => void refresh());
  onBeforeUnmount(() => unsubscribe());

  return { state, debugEnabled, refresh, setDebug, revealLogs, dismissCrash, openDevtools, reportText, copyReport, exportReport, reportProblem };
}