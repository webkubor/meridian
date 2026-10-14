/**
 * Application state: which build this is, what changed in it, whether a newer
 * one exists, and where to send a bug report.
 *
 * Window-scoped rather than repository-scoped, like `useShell` — and for the
 * same reason: none of it belongs to a repository, so it cannot live in a
 * per-repo store.
 *
 * ## What this composable promises about the network
 *
 * Exactly one thing here reaches out: `checkForUpdates`. It asks
 * `github.com/{slug}/releases/latest/download/latest.json` for a version
 * number, and the request carries the app version and nothing else. The
 * release notes are *not* fetched — both changelog files are compiled into the
 * binary by `include_str!`, so reading them is local. Feedback is not sent by
 * this app at all: it builds a URL and hands it to the user's browser, where the
 * user reviews and submits it themselves.
 *
 * That is a brand promise rather than a preference, so it is written down here
 * where a future change would have to argue with it.
 */

import { computed, ref } from "vue";

import type { AppInfo, FeedbackKind, FeedbackTarget, ReleaseNotes, UpdateCheck, UpdateProgress, UpdateStatus } from "../bridges/bindings";
import { updateAvailable } from "../bridges/bindings";
import {
  appFeedbackCompose,
  appInfoLoad,
  appReleaseNotesLoad,
  appUpdateCheck,
  appUpdateInstall,
} from "../bridges/commands";
import { openExternal } from "../bridges/external";
import type { LoadState } from "../stores/loadState";
import { errorState } from "../stores/loadState";
import { markHandled } from "../diagnostics/failures";
import { currentLocale } from "../i18n";

const AUTO_CHECK_KEY = "meridian.updates.autoCheck";

function readAutoCheck(): boolean {
  // Default on: an application that can update itself and never checks is an
  // application that quietly rots. The disclosure in Settings says in full what
  // the check sends, and the switch turns it off for good.
  const stored = window.localStorage.getItem(AUTO_CHECK_KEY);
  return stored === null ? true : stored === "true";
}

/* ---- state --------------------------------------------------------------- */

const info = ref<LoadState<AppInfo>>({ status: "loading" });
const check = ref<LoadState<UpdateCheck>>({ status: "loading" });
const notes = ref<LoadState<ReleaseNotes>>({ status: "loading" });

/** How far the install has got. `idle` until the user asks for it. */
export type InstallPhase = "idle" | "downloading" | "downloaded" | "installing" | "readyToRestart" | "failed";

export interface InstallState {
  phase: InstallPhase;
  received: number;
  total: number | null;
  error: string | null;
}

const install = ref<InstallState>({ phase: "idle", received: 0, total: null, error: null });

const autoCheck = ref(readAutoCheck());
const lastCheckedAt = ref<number | null>(null);
const feedbackTarget = ref<FeedbackTarget | null>(null);
const feedbackBusy = ref(false);

/** Guards against a second check while one is in flight. */
let inFlight: Promise<void> | null = null;

/* ---- derived ------------------------------------------------------------- */

/**
 * The latest known status, if a check has produced one.
 *
 * `notConfigured` and `unsupportedInDebugBuild` have no status, so this is null
 * for them and the UI says why rather than showing a blank version.
 */
const status = computed<UpdateStatus | null>(() => {
  const value = check.value;
  return value.status === "data" && value.data.kind === "checked" ? value.data.status : null;
});

const updateReady = computed(() => (status.value ? updateAvailable(status.value) : false));

const canSelfUpdate = computed(
  () => info.value.status === "data" && info.value.data.canSelfUpdate,
);

const checking = computed(() => check.value.status === "loading");

/** 0–1, or null when the server did not state a total size. */
const installProgress = computed<number | null>(() => {
  const { phase, received, total } = install.value;
  if (phase !== "downloading") return null;
  if (total === null || total <= 0) return null;
  return Math.min(1, received / total);
});

/* ---- actions ------------------------------------------------------------- */

async function loadInfo(): Promise<void> {
  try {
    info.value = { status: "data", data: await appInfoLoad() };
  } catch (error) {
    info.value = errorState(error);
  }
}

/**
 * Reads the changelog for a language. Local: the file is inside the binary.
 * Re-read on every language change so switching to Chinese does not leave the
 * English notes on screen.
 */
async function loadReleaseNotes(locale: string = currentLocale()): Promise<void> {
  notes.value = { status: "loading" };
  try {
    const document = await appReleaseNotesLoad(locale);
    notes.value = { status: "data", data: document };
  } catch (error) {
    notes.value = errorState(error);
  }
}

async function checkForUpdates(): Promise<void> {
  // A check that is already running is the same check; two concurrent requests
  // would race to write `check` and could leave the older answer in place.
  if (inFlight) return inFlight;

  check.value = { status: "loading" };
  const run = (async () => {
    try {
      check.value = { status: "data", data: await appUpdateCheck() };
      lastCheckedAt.value = Date.now();
    } catch (error) {
      check.value = errorState(error);
    } finally {
      inFlight = null;
    }
  })();

  inFlight = run;
  return run;
}

/**
 * Everything the app does once, at mount.
 *
 * The order matters: the info has to arrive before the check can decide whether
 * this build is allowed to make one, and the changelog is read in parallel
 * because it is local and has no bearing on the decision.
 */
async function start(): Promise<void> {
  await Promise.all([loadInfo(), loadReleaseNotes()]);

  // Both conditions are about not making a request with no purpose: a user who
  // turned the check off, and a build that could not act on the answer anyway.
  if (!autoCheck.value || !canSelfUpdate.value) return;
  await checkForUpdates();
}

async function installUpdate(): Promise<void> {
  if (install.value.phase === "downloading" || install.value.phase === "installing") return;

  install.value = { phase: "downloading", received: 0, total: null, error: null };

  const onProgress = (progress: UpdateProgress): void => {
    switch (progress.phase) {
      case "downloading":
        install.value = {
          ...install.value,
          phase: "downloading",
          received: progress.received,
          // Keep the last known total: the updater may omit it on later chunks.
          total: progress.total ?? install.value.total,
        };
        return;
      case "downloaded":
        install.value = { ...install.value, phase: "downloaded" };
        return;
      case "installing":
        install.value = { ...install.value, phase: "installing" };
        return;
      case "readyToRestart":
        install.value = { ...install.value, phase: "readyToRestart" };
        return;
    }
  };

  try {
    await appUpdateInstall(onProgress);
    // The backend's last frame is `readyToRestart`; if the channel dropped one,
    // reaching here still means the install is done.
    if (install.value.phase !== "readyToRestart") {
      install.value = { ...install.value, phase: "readyToRestart" };
    }
  } catch (error) {
    const payload = error as { message?: string } | null;
    install.value = {
      phase: "failed",
      received: install.value.received,
      total: install.value.total,
      error: payload?.message ?? null,
    };
    // The update panel states the failure itself; a second toast would repeat it.
    markHandled(error);
  }
}

/**
 * Builds the issue URL and opens it in the user's browser.
 *
 * The app never posts anything: it composes a URL, the browser opens it, and the
 * user reads and edits the form before pressing Submit. `title` is the only text
 * taken from the user, and it goes into that URL — so it is never secret, which
 * the Feedback panel says out loud.
 */
async function composeFeedback(kind: FeedbackKind, title: string): Promise<FeedbackTarget | null> {
  feedbackBusy.value = true;
  try {
    const target = await appFeedbackCompose({
      kind,
      summary: title.trim() === "" ? null : title.trim(),
    });
    feedbackTarget.value = target;
    await openExternal(target.url);
    return target;
  } catch (error) {
    feedbackTarget.value = null;
    // The About screen shows `feedbackFailed`; claiming the error here keeps the
    // fallback toast from saying the same thing twice.
    markHandled(error);
    return null;
  } finally {
    feedbackBusy.value = false;
  }
}

function setAutoCheck(value: boolean): void {
  autoCheck.value = value;
  try {
    window.localStorage.setItem(AUTO_CHECK_KEY, String(value));
  } catch {
    // Not being able to remember the choice is not a reason to refuse it.
  }
}

/* ---- the composable ------------------------------------------------------ */

export function useApp() {
  return {
    info,
    check,
    status,
    updateReady,
    canSelfUpdate,
    checking,
    install,
    installProgress,
    autoCheck,
    lastCheckedAt,
    notes,
    feedbackTarget,
    feedbackBusy,

    loadInfo,
    loadReleaseNotes,
    checkForUpdates,
    start,
    installUpdate,
    composeFeedback,
    setAutoCheck,
  };
}
