<script setup lang="ts">
import { computed, nextTick, onMounted, provide, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import type {
  BranchInfo,
  CommitInfo,
  DiffPerspective,
  EntryStatus,
  ErrorPayload,
  FeedbackKind,
  HistoryCommitsOptions,
  RepoInfo,
  RepoRoot,
} from "./bridges/bindings";
import type {
  DiffViewMode,
  DialogueRequest,
  DialogueResult,
  HistorySearchModel,
  HistoryViewOptions,
  HunkAction,
  HunkRef,
  LoadState,
  MergeStatus,
  PaletteItemModel,
  RepoSummaryModel,
  ServiceAccount,
  SettingValue,
  SettingsDocument,
  SidebarBadgeModel,
  SidebarRowModel,
  SidebarSectionModel,
  SplitActionOption,
  ToolbarVerbModel,
} from "./components/contracts";
import { useActivity } from "./composables/useActivity";
import { useAppEvents } from "./composables/useAppEvents";
import { useApp } from "./composables/useApp";
import { useBranches } from "./composables/useBranches";
import { toRefChoice, useCompare } from "./composables/useCompare";
import { useDiff } from "./composables/useDiff";
import { useGit } from "./composables/useGit";
import { useHistory } from "./composables/useHistory";
import { useRepositories } from "./composables/useRepositories";
import { useWorkingCopy } from "./composables/useWorkingCopy";
import { useDiagnostics } from "./composables/useDiagnostics";
import type { VerbResult } from "./composables/useGit";
import { claimFailure, configureFailureNet, failureIdFor, reportFailure, type FailureRecord } from "./diagnostics/failures";
import { describeLink } from "./diagnostics/report";
import { OPEN_DIAGNOSTICS } from "./diagnostics/claim";
import { errorText } from "./i18n";
import AboutScreen from "./components/screens/AboutScreen.vue";
import DiagnosticsScreen from "./components/screens/DiagnosticsScreen.vue";
import ActivityScreen from "./components/screens/ActivityScreen.vue";
import AccountScreen from "./components/screens/AccountScreen.vue";
import BlamePane from "./components/panes/BlamePane.vue";
import BranchDetailPane from "./components/panes/BranchDetailPane.vue";
import BranchListPane from "./components/panes/BranchListPane.vue";
import CommitDetailPane from "./components/panes/CommitDetailPane.vue";
import CommandPalette from "./components/palettes/CommandPalette.vue";
import ComparePane from "./components/panes/ComparePane.vue";
import HistoryPane from "./components/panes/HistoryPane.vue";
import RepositoryListScreen from "./components/screens/RepositoryListScreen.vue";
import RepositoryScreen from "./components/screens/RepositoryScreen.vue";
import SettingsScreen from "./components/screens/SettingsScreen.vue";
import WorkingCopyPane from "./components/panes/WorkingCopyPane.vue";
import AppShell from "./layout/AppShell.vue";
import AppSidebar from "./layout/AppSidebar.vue";
import AppToolbar from "./layout/AppToolbar.vue";
import DialogueHost from "./layout/DialogueHost.vue";
import MasterDetail from "./layout/MasterDetail.vue";
import ToastHost from "./layout/ToastHost.vue";
import {
  aboutPath,
  accountPath,
  activityPath,
  blamePath,
  diagnosticsPath,
  diagnosticsFailurePath,
  isWorkspaceTab,
  repositoriesPath,
  repositoryPath,
  settingsPath,
  WORKSPACE_TAB_LABEL_KEYS,
  type WorkspaceTab,
} from "./router/routes";
import { navigate, useRoute } from "./router";
import { currentLocale, setLocale, type Locale } from "./i18n";
import { openExternal } from "./bridges/external";
import { pickDirectory, relaunch } from "./bridges/commands";
import { installFailureLog } from "./diagnostics/logSink";
import {
  askDialogue,
  cancelDialogue,
  resolveDialogue,
  useKeyboardShortcuts,
  usePersistedNumber,
  useShell,
} from "./layout/useShell";

/**
 * The composition root (docs/02 §13). It owns three things and nothing else:
 * the route → screen switch, the wiring of the domain composables into the
 * presentational panes, and the shell services (theme, toasts, dialogues,
 * shortcuts). Every pane below receives props; none of them touches a store.
 */

useAppEvents();

/**
 * The composition root's own copy comes from the catalogue too: everything it
 * shows before a pane takes over (the window subtitle, the sidebar, the
 * toolbar, the command switch, the dialogues and their toasts) is `shell.*`.
 */
const { t } = useI18n();

const shell = useShell();
const route = useRoute();

const app = useApp();
// Destructured because the template only unwraps refs that are top-level
// bindings: `app.info` would render the ref object, `appInfo` renders its value.
const {
  info: appInfo,
  check: appCheck,
  notes: appNotes,
  install: appInstall,
  installProgress,
  autoCheck: autoCheckUpdates,
  lastCheckedAt,
  feedbackBusy,
} = app;
const repositories = useRepositories();
const activity = useActivity();
const compare = useCompare();
const workingCopy = useWorkingCopy();
const branches = useBranches();
const history = useHistory();
const diff = useDiff();
const git = useGit();
const diagnostics = useDiagnostics({
  appInfo: () => appInfo.value.status === "data" ? appInfo.value.data : null,
  routeName: () => route.value.name,
  focusId: () => route.value.query.failure ?? null,
  copy: copyText,
  notify: (text) => shell.pushToast({ kind: "info", text }),
  notifyOpenFailed: (url, reason) => {
    // The browser refused the link, so nothing was sent; the URL goes to the
    // clipboard instead, and the failure is already recorded.
    void navigator.clipboard
      .writeText(url)
      .catch(() => shell.pushToast({ kind: "danger", text: t("shell.toast.copyFailed") }));
    shell.pushToast({ kind: "danger", text: reason || t("shell.toast.linkNotOpened") });
  },
  relaunch: () => relaunch(),
});
const diagnosticsState = diagnostics.state;
const diagnosticsDebugEnabled = diagnostics.debugEnabled;
const crashNote = computed(() => diagnostics.state.value.crash);

/**
 * Where every "详情" control in the app lands: the Diagnostics screen, with
 * the failure expanded when the toast carried its id. The heading takes focus
 * so keyboard and screen-reader users are not left on the old element.
 */
function openDiagnostics(failureId?: string): void {
  navigate(failureId ? diagnosticsFailurePath(failureId) : diagnosticsPath());
  void nextTick(() => {
    document.getElementById("diagnostics-heading")?.focus();
  });
}
provide(OPEN_DIAGNOSTICS, openDiagnostics);

/**
 * Extra chrome for the fallback error toast: the machine code in small text and
 * a one-click route to the diagnostics screen.
 *
 * Kept beside the shell's toast queue rather than inside it because the shell
 * owns *when* a toast disappears, not what a failure wants to say about itself.
 */
const toastMeta = ref<Record<string, { detail?: string; failureId?: string }>>({});

const toastViews = computed(() =>
  shell.toasts.value.map((toast) => ({ ...toast, detail: toastMeta.value[toast.id]?.detail })),
);

/**
 * The last resort: a failure nobody claimed is still shown, with its code and
 * a way into the diagnostics screen focused on it.
 *
 * The headline is the catalogue's sentence for the code (or the finer class),
 * never one generic "the action failed": the engine's own words — a rejected
 * push, a credential the server refused, conflicts to resolve — are the thing
 * that makes the toast actionable, and they ride along in the small line under
 * it.
 */
function showErrorToast(payload: ErrorPayload, failureId?: string): void {
  const headline = payload.code === "unclassified" ? t("diagnostics.internalError") : errorText(payload);
  // The reason line is what tells two failures apart: the engine's own words
  // for a classified failure, the exception's first line for an internal one.
  const reason = payload.code === "unclassified"
    ? payload.detail?.split("\n")[0]
    : payload.message === headline ? undefined : payload.message;
  const detail = [payload.code, reason].filter(Boolean).join(" · ");
  const id = shell.pushToast({
    kind: "danger",
    text: headline,
    actionLabel: t("diagnostics.actions.details"),
  });
  toastMeta.value = { ...toastMeta.value, [id]: { detail, failureId } };
}
configureFailureNet({
  showToast: (payload, record) => showErrorToast(payload, record.id),
  showSummary: (count) => {
    const id = shell.pushToast({ kind: "danger", text: t("diagnostics.actions.suppressed", { count }) });
    toastMeta.value = { ...toastMeta.value, [id]: {} };
  },
});
installFailureLog();

function onToastAction(id: string): void {
  const failureId = toastMeta.value[id]?.failureId;
  if (failureId !== undefined || toastMeta.value[id]) openDiagnostics(failureId);
  onToastDismiss(id);
}

function onToastDismiss(id: string): void {
  if (toastMeta.value[id]) {
    const next = { ...toastMeta.value };
    delete next[id];
    toastMeta.value = next;
  }
  shell.dismissToast(id);
}

/** Diff view mode is shared by the working-copy and commit-detail panes. */
const diffMode = ref<DiffViewMode>("unified");

/* ---- application surface ------------------------------------------------- */
/*
 * Version, updates, release notes and feedback. Window-scoped rather than
 * repository-scoped, so it is wired here at the composition root like the shell
 * services — the About screen receives it all as props and never invokes.
 */

const locale = ref<Locale>(currentLocale());

function changeLocale(next: Locale): void {
  setLocale(next);
  locale.value = next;
  // Both changelog translations are compiled into the binary, so switching
  // language means re-reading the other one rather than re-rendering this one.
  void app.loadReleaseNotes(next);
}

async function onDismissCrash(): Promise<void> {
  await diagnostics.dismissCrash();
}

async function onExportReport(failure: FailureRecord | null): Promise<void> {
  await diagnostics.exportReport(failure);
}

async function restartToUpdate(): Promise<void> {
  // macOS installs the new bundle beside the running one; the process has to
  // come back up to be running it. Nothing else about this session survives,
  // which is why the button is behind an explicit "Ready" state. The same
  // button relaunches for developer tools, which the webview can only add at
  // the next launch.
  await relaunch();
}

async function openExternalLink(url: string): Promise<void> {
  const result = await openExternal(url);
  if (!result.opened) shell.pushToast({ kind: "danger", text: result.reason ?? t("shell.toast.linkFailed") });
}

/** Set when a feedback URL could not be built, so the screen can say so. */
const feedbackFailed = ref(false);

async function onFeedback(kind: FeedbackKind, title: string): Promise<void> {
  const target = await app.composeFeedback(kind, title);
  feedbackFailed.value = target === null;
  if (target === null) return;
  // The browser is already open; this is the record of what went with it.
  shell.pushToast({
    kind: "info",
    text: t("shell.toast.feedbackOpened", { count: target.prefilled.length }),
  });
  const opened = app.lastFeedbackOpen();
  // A link the system would not open is a failure the user must know about:
  // nothing was sent, and the URL is on screen to copy instead.
  if (opened && !opened.opened) {
    shell.pushToast({ kind: "danger", text: opened.reason ?? t("shell.toast.linkNotOpened") });
  }
}

/**
 * The Diagnostics screen's "Report a problem".
 *
 * The report is built and the URL composed first, then a confirmation shows
 * exactly what will be in the link: the query string reaches github.com on the
 * first GET, before anything is submitted, so the review comes before the
 * content leaves the machine. Copy puts the report on the clipboard instead, and
 * cancelling sends nothing at all.
 */
async function onReportProblem(failure: FailureRecord | null): Promise<void> {
  // Built once so the preview, the copy and the link carry the same text.
  const report = diagnostics.reportText(failure);
  const link = await diagnostics.composeReportLink(report);
  if (!link) return;
  const choice = await askDialogue({
    kind: "confirm",
    id: dialogueId(),
    title: t("shell.toast.reportPreviewTitle"),
    message: t("shell.toast.reportPreviewMessage"),
    preview: describeLink(link.url),
    confirmLabel: t("shell.toast.reportOpen"),
    secondaryLabel: t("shell.toast.reportPreviewCopy"),
  });
  if (!choice) return;
  if (choice.value === "secondary") {
    await copyText(report);
    return;
  }
  if (await diagnostics.openReportLink(link)) {
    shell.pushToast({ kind: "info", text: t("shell.toast.reportOpened", { count: link.prefilled.length }) });
  }
}

/** The diagnostics text, or a placeholder when the info has not loaded. */
function appInfoDiagnostics(): string {
  return appInfo.value.status === "data" ? appInfo.value.data.diagnostics : "";
}

onMounted(() => {
  // Loads the build info, reads the bundled changelog, and — only when the
  // stored preference says so and this build could install anything — asks
  // GitHub once. `start` owns that decision rather than the shell, so there is
  // one place that knows when a request is allowed to happen.
  void app.start();
  // The crash note and the debug state are read at boot, not only when the
  // Diagnostics screen is opened: a crash from the previous run has to reach
  // the owner without them going looking for it.
  void diagnostics.refresh();
});

// A failure recorded while the Diagnostics screen is open pulls the freshest
// log tail into it; the tail read at launch predates the failure being looked
// at, which was the complaint.
watch(
  () => diagnostics.state.value.failures.length,
  (count, before) => {
    if (count > (before ?? 0) && route.value.name === "diagnostics") void diagnostics.refresh();
  },
);

watch(
  () => route.value.name,
  (name) => {
    if (name === "diagnostics") void diagnostics.refresh();
  },
);

// HEAD moved but the repository could not be re-read: say so once, and keep
// Push disabled until it can be.
watch(repositories.headStale, (stale) => {
  if (stale) shell.pushToast({ kind: "warning", text: t("shell.toast.headStale") });
});

/* ---- route-derived ------------------------------------------------------- */

const routeName = computed(() => route.value.name);
const activeRepoId = computed(() => route.value.params.repoId ?? repositories.selectedId.value);
const activeTab = computed<WorkspaceTab>(() =>
  isWorkspaceTab(route.value.query.tab) ? route.value.query.tab : "working-copy",
);

const selectedRepo = computed(() => repositories.selected.value);

const summary = computed<RepoSummaryModel | null>(() =>
  selectedRepo.value
    ? {
        info: selectedRepo.value,
        headLabel: selectedRepo.value.headBranch,
        state: selectedRepo.value.state,
      }
    : null,
);

const windowTitle = computed(() => {
  const repo = selectedRepo.value;
  if (!repo) return "Meridian";
  const parts = (repo.workdir ?? repo.gitdir).split("/").filter(Boolean);
  return parts[parts.length - 1] ?? "Meridian";
});

const windowSubtitle = computed(() => {
  if (routeName.value === "settings") return t("shell.subtitle.settings");
  if (routeName.value === "account") return t("shell.subtitle.account");
  if (routeName.value === "about") return t("shell.subtitle.about");
  if (routeName.value === "diagnostics") return t("diagnostics.title");
  if (routeName.value === "repositories") return t("shell.subtitle.repositories");
  const head = selectedRepo.value?.headBranch;
  return head ? t("shell.subtitle.on", { branch: head }) : selectedRepo.value?.gitdir ?? null;
});

/* ---- sidebar ------------------------------------------------------------- */

const sidebarWidth = usePersistedNumber("meridian.sidebarWidth", 216, 180, 340);
const sidebarFilter = ref("");
const selectedSidebarId = ref<string | null>(null);

const localBranches = computed<BranchInfo[]>(() =>
  branches.branches.value.filter((branch) => !branch.isRemote),
);

/**
 * The error caption a sidebar section shows instead of "nothing here".
 *
 * "No commits yet" is a statement about the repository, and a failed load is
 * not one — the empty caption would be a false thing to show.
 */
function sectionError(state: LoadState<unknown>): string | undefined {
  return state.status === "error" ? errorText(state.error) : undefined;
}

const failedTabs = computed<WorkspaceTab[]>(() => {
  const failed: WorkspaceTab[] = [];
  if (workingCopy.state.value.status === "error") failed.push("working-copy");
  if (history.state.value.status === "error") failed.push("history");
  if (branches.state.value.status === "error") failed.push("branches");
  if (compare.state.value.status === "error") failed.push("compare");
  return failed;
});

const workingCopyTotal = computed(() => {
  const value = workingCopy.summary.value;
  if (!value) return 0;
  return value.staged + value.unstaged + value.untracked + value.conflicted;
});

function badgeTone(count: number, danger = false): "danger" | "warning" | "neutral" {
  if (count === 0) return "neutral";
  return danger ? "danger" : "warning";
}

const sidebarSections = computed<SidebarSectionModel[]>(() => {
  const needle = sidebarFilter.value.trim().toLowerCase();
  const keep = (label: string): boolean =>
    needle === "" || label.toLowerCase().includes(needle);

  const branchRows: SidebarRowModel[] = branches.branches.value
    .filter((branch) => !branch.isRemote && keep(branch.name))
    .map((branch) => {
      const badges: SidebarBadgeModel[] = [];
      if (branch.ahead > 0) badges.push({ id: "ahead", icon: "arrow.up", label: String(branch.ahead), tone: "positive" });
      if (branch.behind > 0) badges.push({ id: "behind", icon: "arrow.down", label: String(branch.behind), tone: "info" });
      return {
        id: `branch:${branch.fullName}`,
        label: branch.name,
        icon: branch.isHead ? "checkmark.circle.fill" : "arrow.triangle.branch",
        badges,
        current: branch.isHead,
        hasOptions: true,
      };
    });

  const remoteRows: SidebarRowModel[] = branches.branches.value
    .filter((branch) => branch.isRemote && keep(branch.name))
    .map((branch) => ({
      id: `branch:${branch.fullName}`,
      label: branch.name,
      icon: "cloud",
      badges: [] as SidebarBadgeModel[],
    }));

  return [
    {
      id: "working-copy",
      title: t("shell.sidebar.workingCopy"),
      icon: "nav.workingCopy",
      pseudo: true,
      collapsed: false,
      emptyCaption: t("shell.sidebar.emptyWorkingCopy"),
      errorCaption: sectionError(workingCopy.state.value),
      rows: [
        {
          id: "sidebar:working-copy",
          label: t("shell.sidebar.rowWorkingCopy"),
          icon: "doc.text",
          badges:
            workingCopyTotal.value > 0
              ? [
                  {
                    id: "wc-count",
                    label: String(workingCopyTotal.value),
                    tone: badgeTone(workingCopyTotal.value),
                  },
                ]
              : [],
          current: activeTab.value === "working-copy",
          hasOptions: false,
        },
      ],
    },
    {
      id: "history",
      title: t("shell.sidebar.history"),
      icon: "nav.history",
      collapsed: false,
      emptyCaption: t("shell.sidebar.emptyHistory"),
      errorCaption: sectionError(history.state.value),
      // The filter matches what the section is *called* on screen, not an
      // English literal — otherwise typing 中文 in the filter finds nothing.
      rows:
        history.commits.value.length > 0 && keep(t("shell.sidebar.history"))
          ? [
              {
                id: "sidebar:history",
                label: t("shell.sidebar.rowHistory"),
                icon: "clock",
                detail: String(history.commits.value.length),
                badges: [],
                current: activeTab.value === "history",
                hasOptions: false,
              },
            ]
          : [],
    },
    {
      id: "branches",
      title: t("shell.sidebar.branches"),
      icon: "nav.branch",
      collapsed: false,
      emptyCaption: t("shell.sidebar.emptyBranches"),
      errorCaption: sectionError(branches.state.value),
      rows: branchRows,
    },
    {
      id: "remotes",
      title: t("shell.sidebar.remotes"),
      icon: "cloud",
      collapsed: true,
      emptyCaption: t("shell.sidebar.emptyRemotes"),
      errorCaption: repositories.remotesError.value ? errorText(repositories.remotesError.value) : undefined,
      rows: remoteRows,
    },
    {
      id: "tags",
      title: t("shell.sidebar.tags"),
      icon: "nav.tags",
      collapsed: true,
      emptyCaption: t("shell.sidebar.emptyTags"),
      rows: [],
    },
    {
      id: "stashes",
      title: t("shell.sidebar.stashes"),
      icon: "nav.stashes",
      collapsed: true,
      emptyCaption: t("shell.sidebar.emptyStashes"),
      rows: [],
    },
  ];
});

const collapsedSections = ref<Record<string, boolean>>({});

const sectionsWithCollapse = computed<SidebarSectionModel[]>(() =>
  sidebarSections.value.map((section) => ({
    ...section,
    collapsed: collapsedSections.value[section.id] ?? section.collapsed,
  })),
);

function toggleSidebarSection(id: string): void {
  const current = collapsedSections.value[id];
  const fallback = sidebarSections.value.find((section) => section.id === id)?.collapsed ?? false;
  collapsedSections.value = { ...collapsedSections.value, [id]: !(current ?? fallback) };
}

const recents = computed<RepoSummaryModel[]>(() =>
  repositories.repositories.value.map((info) => ({
    info,
    headLabel: info.headBranch,
    state: info.state,
  })),
);

/** The launch screen's recents filter — separate from the sidebar's. */
const recentsFilter = ref("");

/** Remove a repository from the recent list (it is closed, not deleted on disk). */
function onRemoveRecent(id: string): void {
  repositories.remove(id);
}

/** Reveal a repository's working directory in the OS file manager. */
function onRevealRecent(id: string): void {
  void runVerb(() => repositories.reveal(id), t("shell.toast.revealed"));
}

function onSelectSidebarRow(id: string): void {
  selectedSidebarId.value = id;
  if (id === "sidebar:working-copy") {
    goTab("working-copy");
    return;
  }
  if (id === "sidebar:history") {
    goTab("history");
    return;
  }
  if (id.startsWith("branch:")) {
    selectedBranchName.value = id.slice("branch:".length);
    goTab("branches");
  }
}

/* ---- navigation ---------------------------------------------------------- */

function goTab(tab: WorkspaceTab): void {
  const id = activeRepoId.value;
  if (!id) {
    navigate(repositoriesPath());
    return;
  }
  navigate(repositoryPath(id, tab));
}

/**
 * Fill a `path`-kind setting from the folder picker.
 *
 * A cancelled picker leaves the setting exactly as it was — the row keeps its
 * old value and no error appears, because nothing went wrong.
 */
async function onChooseSettingPath(key: string): Promise<void> {
  const picked = await pickDirectory();
  if (picked) setSetting(key, picked);
}

/**
 * Seed the compare tab the first time it is opened in a repository: the
 * checked-out branch against what it tracks, else against `main`/`master`.
 *
 * Opening the tab onto two empty pickers makes the empty state the first thing
 * everyone sees and the useful one something you have to discover. Only seeds
 * when nothing is chosen, so it never overrides the user's own pick.
 */
function seedCompareDefaults(): void {
  if (activeTab.value !== "compare" || routeName.value !== "repository") return;
  if (compare.left.value !== null || compare.right.value !== null) return;
  const all = branches.allBranches.value;
  const head = all.find((entry) => entry.isHead);
  if (!head) return;
  const base =
    (head.upstream ? all.find((entry) => entry.isRemote && entry.name === head.upstream) : undefined) ??
    all.find((entry) => !entry.isRemote && !entry.isHead && (entry.name === "main" || entry.name === "master"));
  if (!base) return;
  compare.setBoth(toRefChoice(base), toRefChoice(head));
}

watch([activeTab, () => branches.allBranches.value], seedCompareDefaults);

/** Pick one side of the comparison. */
async function pickCompareRef(side: "left" | "right"): Promise<void> {
  const result = await askDialogue({
    kind: "chooseBranch",
    id: dialogueId(),
    title:
      side === "left"
        ? t("shell.dialogue.compareBaseTitle")
        : t("shell.dialogue.compareTargetTitle"),
    branches: branches.allBranches.value,
    current: side === "left" ? compare.left.value?.id ?? null : compare.right.value?.id ?? null,
    emitFullName: true,
  });
  if (result?.kind !== "chooseBranch") return;
  const branch = branches.allBranches.value.find((entry) => entry.fullName === result.value);
  if (!branch) return;
  compare.choose(side, toRefChoice(branch));
}

/**
 * Push the branch this pane is showing.
 *
 * The bare `git.push()` publishes whatever HEAD is on, so pressing Push while
 * reading another branch's pane pushes the wrong ref — silently, and to a
 * remote. The pane's own branch is the one the user is looking at.
 */
async function pushSelectedBranch(): Promise<void> {
  const branch = selectedBranchInfo.value;
  const name = branch?.name ?? null;
  if (!name) {
    shell.pushToast({ kind: "warning", text: t("shell.toast.noBranchSelected") });
    return;
  }
  await runVerb(
    () => git.push(name),
    t("shell.toast.pushed", { branch: name }),
  );
}

/** The remote-only branch pane's verb: create the local branch and check it out. */
async function checkoutSelectedBranch(): Promise<void> {
  const branch = selectedBranchInfo.value;
  if (!branch?.isRemote) return;
  if (!(await confirmDirtyCheckout())) return;
  const ok = await branches.checkoutRemote(branch.fullName);
  if (ok) {
    shell.pushToast({ kind: "success", text: t("shell.toast.checkedOut", { branch: branch.name }) });
  } else if (branches.lastError.value) {
    reportError(branches.lastError.value);
  }
}

function onOpenCommitFromCompare(oid: string): void {
  void history.select(oid);
}

/** Compare is repository-scoped: it must not answer for the previous repo. */
watch(
  activeRepoId,
  (id) => compare.forRepo(id ?? null),
  { immediate: true },
);

function openRepository(id: string): void {
  repositories.select(id);
  navigate(repositoryPath(id));
}

/**
 * Leave the launch screen for a repository that just opened.
 *
 * Opening is only half of it: staying put afterwards is the difference between
 * "it worked" and "did anything happen?".
 */
async function openAfter(opened: Promise<RepoInfo | null>): Promise<void> {
  const info = await opened;
  if (info) navigate(repositoryPath(info.id));
}

function onOpenRequested(path: string): void {
  void openAfter(repositories.open(path));
}

/* ---- shell helpers ------------------------------------------------------- */

/**
 * Report a failure the caller is about to show itself: recorded and logged,
 * then a toast — claimed, so the net does not raise a second one for the same
 * failure.
 */
function reportError(error: unknown): void {
  const payload = claimFailure(error, "ui");
  showErrorToast(payload, failureIdFor(payload));
}

/** Copy text (hashes, branch names, keys) to the clipboard. */
async function copyText(text: string): Promise<void> {
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    shell.pushToast({ kind: "success", text: t("shell.toast.copied") });
  } catch {
    shell.pushToast({ kind: "danger", text: t("shell.toast.copyFailed") });
  }
}

/**
 * Run a repo-level git verb, surfacing the engine's own error or a success note
 * — and recording it in the activity list.
 *
 * `labelKey` is passed rather than the rendered text so the entry re-labels
 * itself when the language changes, and so the list holds a name instead of a
 * snapshot of one taken a moment before the user switched.
 */
async function runVerb(
  action: () => Promise<VerbResult | ErrorPayload | null>,
  success: string,
  labelKey = "shell.activity.genericOperation",
): Promise<void> {
  const id = activity.start(labelKey);
  const outcome = await action();
  const error = outcome && "error" in outcome ? outcome.error : (outcome as ErrorPayload | null);
  if (error) {
    // The activity list is read by the person who just clicked; its reason is
    // the same sentence the toast shows, not the backend's English.
    activity.finish(id, "failed", error.code === "unclassified" ? error.message : errorText(error));
    reportError(error);
    return;
  }
  const conflicted = outcome !== null && "conflicted" in outcome && outcome.conflicted;
  activity.finish(id, conflicted ? "failed" : "done");
  if (conflicted) {
    // `merge_start`/`rebase_start` come back `Ok` with `outcome: conflicted`
    // when conflicts stopped them: a success toast here would say "merged" for
    // an operation that is waiting for a human.
    shell.pushToast({ kind: "warning", text: t("shell.toast.conflicted") });
    goTab("working-copy");
    return;
  }
  shell.pushToast({ kind: "success", text: success });
}

/* ---- toolbar ------------------------------------------------------------- */

const toolbarVerbs = computed<ToolbarVerbModel[]>(() => {
  const hasRepo = selectedRepo.value !== null;
  const hasRemote = repositories.defaultRemote.value !== null;
  const hasHead = selectedRepo.value?.headBranch != null;

  const repoReason = hasRepo ? undefined : t("shell.toolbar.noRepository");
  const remoteReason = !hasRepo
    ? t("shell.toolbar.noRepository")
    : hasRemote
      ? undefined
      : t("shell.toolbar.noRemote");
  const pushReason = !hasRepo
    ? t("shell.toolbar.noRepository")
    : repositories.headStale.value
      ? t("shell.toast.pushStale")
      : !hasRemote
        ? t("shell.toolbar.noRemote")
        : !hasHead
          ? t("shell.toolbar.detachedHead")
          : undefined;

  const branchOptions: SplitActionOption[] = [
    { id: "branch.new", label: t("shell.toolbar.newBranch"), disabled: !hasRepo },
    { id: "branch.checkout", label: t("shell.toolbar.checkoutBranch"), disabled: !hasRepo },
    { id: "branch.rename", label: t("shell.toolbar.renameBranch"), disabled: !hasRepo },
  ];

  return [
    {
      id: "branch",
      label: t("shell.toolbar.branch"),
      icon: "action.branch",
      group: "global",
      disabled: !hasRepo,
      reason: repoReason,
      options: branchOptions,
    },
    { id: "fetch", label: t("shell.toolbar.fetch"), icon: "action.fetch", group: "global", disabled: remoteReason !== undefined, reason: remoteReason },
    { id: "pull", label: t("shell.toolbar.pull"), icon: "action.pull", group: "global", disabled: remoteReason !== undefined, reason: remoteReason },
    { id: "push", label: t("shell.toolbar.push"), icon: "action.push", group: "global", disabled: pushReason !== undefined, reason: pushReason },
    { id: "stash", label: t("shell.toolbar.stash"), icon: "action.stash", group: "global", disabled: !hasRepo, reason: repoReason },

    { id: "quickOpen", label: t("shell.toolbar.quickOpen"), icon: "action.quickOpen", group: "utility", disabled: false },
    { id: "quickActions", label: t("shell.toolbar.quickActions"), icon: "action.quickActions", group: "utility", disabled: false },
    // Always present, not only while the debug chip is lit: this screen is
    // where a failure, a crash note and a report live, and "open it after a
    // crash" must not depend on a mode the user has not turned on.
    { id: "diagnostics", label: t("diagnostics.open"), icon: "exclamationmark.bubble", group: "utility", disabled: false },
    { id: "settings", label: t("shell.toolbar.settings"), icon: "nav.settings", group: "utility", disabled: false },
    { id: "account", label: t("shell.toolbar.account"), icon: "nav.services", group: "utility", disabled: false },
    { id: "about", label: t("shell.toolbar.about"), icon: "info.circle", group: "utility", disabled: false },
  ];
});

/**
 * The chip counts what is actually running.
 *
 * It used to be hardcoded to `unavailable: true` with a "not in this build"
 * reason, which left a permanently greyed-out control in the toolbar — the
 * clearest single example of a button that looks like part of the app and
 * isn't. There is now a real list behind it; see `useActivity`.
 */
const activitySummary = computed(() => ({
  count: activity.running.value.length,
  busy: activity.running.value.length > 0,
  // "Stalled" is not knowable from here: nothing reports per-operation
  // progress, and a guessed threshold would fire on slow networks and quiet
  // the user for a fetch that was about to finish.
  stalled: false,
  unavailable: false,
  unavailableReason: undefined as string | undefined,
}));

async function onToolbarRun(id: string): Promise<void> {
  switch (id) {
    case "branch":
      await newBranchFlow();
      return;
    case "fetch":
      await runVerb(() => git.fetch(), t("shell.toast.fetched"));
      return;
    case "pull":
      await runVerb(() => git.pull(), t("shell.toast.pulled"));
      return;
    case "push": {
      // The toast names the branch that was published; without it the sentence
      // renders with an empty name, which is worse than no sentence.
      const branch = repositories.selected.value?.headBranch ?? "";
      await runVerb(
        () => git.push(branch || undefined),
        t("shell.toast.pushed", { branch }),
      );
      return;
    }
    case "stash":
      await runVerb(() => git.stash(), t("shell.toast.stashed"));
      return;
    case "quickOpen":
      openPalette("open");
      return;
    case "quickActions":
      openPalette("actions");
      return;
    case "settings":
      navigate(settingsPath());
      return;
    case "account":
      navigate(accountPath());
      return;
    case "diagnostics":
      openDiagnostics();
      return;
    case "about":
      navigate(aboutPath());
      return;
    case "activity":
      navigate(activityPath());
      return;
    default:
      return;
  }
}

async function onToolbarChoose(verbId: string, optionId: string): Promise<void> {
  if (verbId !== "branch") return;
  if (optionId === "branch.new") await newBranchFlow();
  else if (optionId === "branch.checkout") await checkoutFlow();
  else if (optionId === "branch.rename") await renameBranchFlow();
}

/* ---- dialogue flows ------------------------------------------------------ */

function dialogueId(): string {
  return `dlg-${Date.now()}-${Math.round(Math.random() * 1000)}`;
}

async function newBranchFlow(): Promise<void> {
  const request: DialogueRequest = {
    kind: "prompt",
    id: dialogueId(),
    title: t("shell.dialogue.newBranchTitle"),
    label: t("shell.dialogue.newBranchLabel"),
    value: "",
    // A git ref example, not copy: `feature/…` is the convention every Git
    // user reads, so it stays as typed rather than becoming a translated
    // string that no longer looks like a branch name.
    placeholder: "feature/short-description",
    helpText: t("shell.dialogue.newBranchHelp"),
    validate: "refname",
    option: { label: t("shell.dialogue.newBranchCheckoutOption"), checked: true },
  };
  const result = await askDialogue(request);
  if (!result || result.value.trim() === "") return;
  // `optionChecked` is the "check out after creation" tick; it has to reach the
  // command, or unticking it does nothing at all.
  const created = await branches.create(result.value.trim(), result.optionChecked);
  if (created) {
    shell.pushToast({ kind: "success", text: t("shell.toast.branchCreated", { branch: created.name }) });
  } else if (branches.lastError.value) {
    reportError(branches.lastError.value);
  }
}

/**
 * What the sidebar's per-section "+" adds. Keys, not nouns: the noun has to
 * agree with the sentence built around it, which is a different string in each
 * language.
 */
const SECTION_ADD_NOUN_KEYS: Record<string, string> = {
  "working-copy": "shell.unavailable.nounWorkingCopy",
  history: "shell.unavailable.nounCommits",
  remotes: "shell.unavailable.nounRemotes",
  tags: "shell.unavailable.nounTags",
  stashes: "shell.unavailable.nounStashes",
};

/**
 * The sidebar "+" is offered on every section, but only Branch can create
 * something in this build (docs/03 §4). Rather than open the New-branch dialog
 * from a Tags or Stashes header — which would be a lie about what the button
 * does — say plainly that the section cannot add yet.
 */
async function onSectionAdd(sectionId: string): Promise<void> {
  if (sectionId === "branches") {
    await newBranchFlow();
    return;
  }
  shell.pushToast({
    kind: "info",
    text: t("shell.unavailable.addSection", {
      noun: t(SECTION_ADD_NOUN_KEYS[sectionId] ?? "shell.unavailable.nounEntries"),
    }),
  });
}

async function renameBranchFlow(preset?: string | null): Promise<void> {
  const name = preset ?? selectedBranchName.value ?? selectedRepo.value?.headBranch ?? null;
  if (!name) return;
  const branch = branches.branches.value.find((entry) => entry.name === name || entry.fullName === name);
  const oldName = branch?.name ?? name;

  const result = await askDialogue({
    kind: "prompt",
    id: dialogueId(),
    title: t("shell.dialogue.renameBranchTitle"),
    label: t("shell.dialogue.renameBranchLabel"),
    value: oldName,
    // A git ref example; see the note in `newBranchFlow`.
    placeholder: "feature/renamed",
    validate: "refname",
  });
  if (!result || result.value.trim() === "" || result.value.trim() === oldName) return;

  const renamed = await branches.rename(oldName, result.value.trim());
  if (renamed) {
    selectedBranchName.value = renamed.fullName;
    shell.pushToast({ kind: "success", text: t("shell.toast.branchRenamed", { branch: renamed.name }) });
  } else if (branches.lastError.value) {
    reportError(branches.lastError.value);
  }
}

/** Merge `name` into the current branch (docs/03 §12 `merge_start`). */
async function mergeBranch(name: string | null): Promise<void> {
  if (!name) return;
  await runVerb(() => git.merge(name), t("shell.toast.merged", { branch: name }));
}

/** Rebase the current branch onto `name` (docs/03 §12 `rebase_start`). */
async function rebaseBranch(name: string | null): Promise<void> {
  if (!name) return;
  await runVerb(() => git.rebase(name), t("shell.toast.rebased", { branch: name }));
}

/** Delete a branch behind an explicit confirmation (docs/03 §4 `branch_delete`). */
async function deleteBranch(name: string | null): Promise<void> {
  if (!name) return;
  const branch = branches.branches.value.find((entry) => entry.name === name || entry.fullName === name);
  const label = branch?.name ?? name;
  const confirmed = await askDialogue({
    kind: "confirm",
    id: dialogueId(),
    title: t("shell.dialogue.deleteBranchTitle"),
    message: t("shell.dialogue.deleteBranchMessage", { branch: label }),
    items: [label],
    confirmLabel: t("shell.dialogue.deleteBranchConfirm"),
    danger: true,
  });
  if (!confirmed) return;

  const ok = await branches.remove(branch?.name ?? name);
  if (ok) shell.pushToast({ kind: "success", text: t("shell.toast.branchDeleted", { branch: label }) });
  else if (branches.lastError.value) reportError(branches.lastError.value);
}

/**
 * Switch branches.
 *
 * One picker, every kind of ref, one verb underneath: a remote-tracking ref has
 * no local branch to switch to, so it is checked out as a new tracking branch
 * instead of failing with "branch not found".
 */
async function checkoutFlow(preset?: string): Promise<void> {
  const name =
    preset ??
    (await askDialogue({
      kind: "chooseBranch",
      id: dialogueId(),
      title: t("shell.dialogue.checkoutTitle"),
      branches: branches.allBranches.value,
      current: selectedRepo.value?.headBranch ?? null,
      emitFullName: true,
    }))?.value;
  if (!name) return;

  const branch = branches.allBranches.value.find(
    (entry) => entry.fullName === name || entry.name === name,
  );
  if (branch?.isRemote === true) {
    // Creating a branch changes what exists on disk, so the dirty-tree
    // confirmation applies exactly as it does to switching.
    if (!(await confirmDirtyCheckout())) return;
    const created = await branches.checkoutRemote(branch.fullName);
    if (created) {
      shell.pushToast({
        kind: "success",
        text: t("shell.toast.checkedOut", { branch: branch.name }),
      });
    } else if (branches.lastError.value) {
      reportError(branches.lastError.value);
    }
    return;
  }

  // The setting is remembered because "ask me every time" and "never ask" are
  // both real preferences; honouring neither would make the switch a lie.
  if (confirmCheckout.value && !(await confirmDirtyCheckout())) return;

  const ok = await branches.checkout(branch?.name ?? name);
  if (ok) shell.pushToast({ kind: "success", text: t("shell.toast.checkedOut", { branch: name }) });
  else if (branches.lastError.value) reportError(branches.lastError.value);
}

/** The dirty-tree confirmation. True when there is nothing to confirm. */
async function confirmDirtyCheckout(): Promise<boolean> {
  if (workingCopyTotal.value === 0) return true;
  const confirmed = await askDialogue({
    kind: "confirm",
    id: dialogueId(),
    title: t("shell.dialogue.checkoutDirtyTitle"),
    message: t("shell.dialogue.checkoutDirtyMessage", { count: workingCopyTotal.value }),
    items: workingCopy.entries.value.slice(0, 8).map((entry) => entry.path),
    confirmLabel: t("shell.dialogue.checkoutDirtyConfirm"),
  });
  return Boolean(confirmed);
}

/**
 * Clear the assume-unchanged bit on one file.
 *
 * No confirmation: it makes a file visible again, which is the safe direction,
 * and the alternative is a flag a user cannot undo from the interface.
 */
async function clearAssumeUnchanged(path: string): Promise<void> {
  const ok = await workingCopy.setAssumeUnchanged([path], false);
  if (!ok && workingCopy.lastError.value) {
    reportError(workingCopy.lastError.value);
  }
}

async function discardEntry(path: string): Promise<void> {
  const confirmed = await askDialogue({
    kind: "confirm",
    id: dialogueId(),
    title: t("shell.dialogue.discardTitle"),
    message: t("shell.dialogue.discardMessage"),
    items: [path],
    confirmLabel: t("shell.dialogue.discardConfirm"),
    danger: true,
  });
  if (!confirmed) return;
  const includeUntracked = workingCopy.entries.value.some(
    (entry) => entry.path === path && entry.code === "untracked",
  );
  const ok = await workingCopy.discard(path, includeUntracked);
  if (ok) shell.pushToast({ kind: "success", text: t("shell.toast.discarded") });
  else reportError(workingCopy.lastError.value);
}

/* ---- working copy -------------------------------------------------------- */

const selectedPath = ref<string | null>(null);
const detailWidth = usePersistedNumber("meridian.detailWidth", 460, 300, 900);

const selectedEntry = computed<EntryStatus | null>(() => {
  const path = selectedPath.value;
  if (!path) return null;
  return workingCopy.entries.value.find((entry) => entry.path === path) ?? null;
});

const conflictedOnly = ref(false);
const composerSubject = ref("");
const composerBody = ref("");
const composerAmend = ref(false);
const composerSignOff = ref(false);

const commitOptions: SplitActionOption[] = [
  { id: "commit.push", label: t("shell.commitAndPush") },
];

const committing = ref(false);

/** Which diff the engine should render for a working-copy entry. */
function perspectiveFor(entry: EntryStatus): DiffPerspective {
  if (entry.code === "untracked") return "workingTreeToBase";
  if (entry.staged) return "indexToBase";
  return "workingTreeToIndex";
}

/**
 * Keep the diff pane fed from the real command: whenever the selected file (or
 * which side of the index it is on) changes, load its diff. A stale document is
 * cleared the moment nothing is selected.
 */
watch(
  () => {
    const entry = selectedEntry.value;
    return entry ? `${entry.path}\u0000${entry.staged}\u0000${entry.code}` : "";
  },
  () => {
    const entry = selectedEntry.value;
    if (!entry) {
      diff.clear();
      return;
    }
    void diff.load(entry.path, perspectiveFor(entry));
  },
  { immediate: true },
);

async function onHunkAction(payload: { action: HunkAction; hunk: HunkRef; path: string }): Promise<void> {
  await diff.runHunk(payload.path, payload.hunk, payload.action);
  await workingCopy.refresh();
}

/** Assemble the composer's subject + body into the commit message. */
function composeMessage(): string {
  const subject = composerSubject.value.trim();
  const body = composerBody.value.trim();
  if (!subject) return body;
  return body ? `${subject}\n\n${body}` : subject;
}

async function commitNow(): Promise<boolean> {
  const message = composeMessage();
  const staged = (workingCopy.summary.value?.staged ?? 0) > 0;
  // An amend may reuse the existing message when only the staged content
  // changes it (`git commit --amend --no-edit`); otherwise a message is needed.
  const reusesMessage = composerAmend.value && message === "" && staged;
  if (message === "" && !reusesMessage) {
    shell.pushToast({ kind: "warning", text: t("shell.toast.commitMessageRequired") });
    return false;
  }
  committing.value = true;
  const ok = await workingCopy.commit({
    message,
    amend: composerAmend.value,
    signOff: composerSignOff.value,
  });
  committing.value = false;
  if (ok) {
    composerSubject.value = "";
    composerBody.value = "";
    composerAmend.value = false;
    shell.pushToast({ kind: "success", text: t("shell.toast.committed") });
    await history.refresh();
  } else {
    reportError(workingCopy.lastError.value);
  }
  return ok;
}

function onCommit(): void {
  void commitNow();
}

async function onCommitChoose(optionId: string): Promise<void> {
  if (optionId === "commit.push") {
    if (await commitNow()) await runVerb(() => git.push(), t("shell.toast.committedAndPushed"));
  }
}

function onResolveConflict(path: string): void {
  void runVerb(() => git.resolveConflict([path]), t("shell.toast.conflictResolved"));
}

/** Merge/operation status is owned by `stores/workingCopy` (port pending). */
const mergeStatus = ref<MergeStatus | null>(null);

/* ---- history selection --------------------------------------------------- */

function onHistorySelect(commit: CommitInfo): void {
  void history.select(commit.oid);
}

function onHistoryRefClick(payload: { commit: CommitInfo }): void {
  void history.select(payload.commit.oid);
}

/* ---- history ------------------------------------------------------------- */

const historyView = ref<HistoryViewOptions>({
  showGraph: true,
  density: "regular",
  sort: "date",
  firstParent: false,
  dateGrouping: "day",
});
const historySearch = ref<HistorySearchModel>({
  text: "",
  author: "",
  since: "",
  until: "",
  type: "all",
});

/** The engine query the history search box and view toggles map onto. */
function historyQuery(): HistoryCommitsOptions {
  const text = historySearch.value.text.trim();
  return {
    sort: historyView.value.sort,
    firstParent: historyView.value.firstParent,
    // `SearchMode::All` matches message, author and hash prefix — exactly what
    // the search box advertises.
    search: text === "" ? undefined : text,
    searchMode: "all",
  };
}

// Re-run the load whenever a filter the engine understands changes. Density,
// date-grouping and show-graph are view-only and deliberately excluded.
watch(
  () => [historyView.value.sort, historyView.value.firstParent, historySearch.value.text] as const,
  () => void history.setQuery(historyQuery()),
);

/** The engine has no merge/non-merge predicate; say so instead of faking it. */
const historyTypeUnavailable = computed(() => t("shell.unavailable.historyMergeFilter"));

/* ---- branches ------------------------------------------------------------ */

const selectedBranchName = ref<string | null>(null);
const branchFilter = ref<"local" | "remote" | "all">("local");
const branchSearch = ref("");

const selectedBranchInfo = computed(() => {
  const name = selectedBranchName.value;
  if (!name) return null;
  // Local *and* remote: the list pane shows both, so a selected remote row that
  // resolves against the local-only list looks selectable and selects nothing.
  const branch = branches.allBranches.value.find((entry) => entry.fullName === name) ?? null;
  return branch ? { ...branch, fullyMerged: undefined, lastAuthor: undefined } : null;
});

/* ---- settings ------------------------------------------------------------ */

const SETTINGS_KEY = "meridian.settings";
const storedSettings = ref<Record<string, SettingValue>>(readStoredSettings());

function readStoredSettings(): Record<string, SettingValue> {
  try {
    const raw = window.localStorage.getItem(SETTINGS_KEY);
    return raw ? (JSON.parse(raw) as Record<string, SettingValue>) : {};
  } catch {
    return {};
  }
}

function settingValue<T extends SettingValue>(key: string, fallback: T): T {
  const value = storedSettings.value[key];
  return (typeof value === typeof fallback ? (value as T) : fallback) ?? fallback;
}

const editorFontSize = computed(() => Number(settingValue("shell.editorFontSize", 13)));
const confirmCheckout = computed(() => Boolean(settingValue("shell.confirmCheckout", true)));

watch(
  editorFontSize,
  (size) => {
    document.documentElement.style.setProperty("--md-font-size-editor", `${size}px`);
  },
  { immediate: true },
);

function setSetting(key: string, value: SettingValue): void {
  storedSettings.value = { ...storedSettings.value, [key]: value };
  persistSettings();
  if (key === "shell.appearance") shell.setThemeMode(value as "auto" | "light" | "dark");
}

/**
 * Settings carry privacy and safety meanings (confirm before switching, theme),
 * so a failed write is reported rather than swallowed: the choice would be
 * forgotten at the next launch without the user ever knowing.
 */
function persistSettings(): void {
  try {
    window.localStorage.setItem(SETTINGS_KEY, JSON.stringify(storedSettings.value));
  } catch (error) {
    reportFailure(error, "settings.persist");
    shell.pushToast({ kind: "warning", text: t("shell.toast.settingsNotSaved") });
  }
}

function resetSetting(key: string): void {
  const next = { ...storedSettings.value };
  delete next[key];
  storedSettings.value = next;
  persistSettings();
}

const settingsDocument = computed<SettingsDocument>(() => ({
  version: 1,
  tabs: [
    {
      id: "appearance",
      label: t("shell.settings.appearance"),
      icon: "sun.max",
      source: "shell",
      groups: [
        {
          id: "editor",
          label: t("shell.settings.editor"),
          rows: [
            {
              key: "shell.editorFontSize",
              label: t("shell.settings.editorFontSize"),
              kind: "int",
              value: editorFontSize.value,
              defaultValue: 13,
              min: 10,
              max: 22,
              help: t("shell.settings.editorFontSizeHelp"),
              shellOwned: true,
            },
          ],
        },
      ],
    },
    {
      id: "git",
      label: t("shell.settings.git"),
      icon: "action.branch",
      source: "shell",
      groups: [
        {
          id: "safety",
          label: t("shell.settings.safety"),
          rows: [
            {
              key: "shell.confirmCheckout",
              label: t("shell.settings.confirmCheckout"),
              kind: "bool",
              value: confirmCheckout.value,
              defaultValue: true,
              help: t("shell.settings.confirmCheckoutHelp"),
              shellOwned: true,
            },
          ],
        },
      ],
    },
    {
      id: "general",
      label: t("shell.settings.general"),
      icon: "gearshape",
      source: "schema",
      groups: [],
    },
    {
      id: "services",
      label: t("shell.settings.services"),
      icon: "cloud",
      source: "schema",
      groups: [],
    },
    {
      id: "advanced",
      label: t("shell.settings.advanced"),
      icon: "wand.and.stars",
      source: "schema",
      groups: [],
    },
  ],
}));

const settingsTab = computed(() => route.value.query.section ?? "appearance");
const settingsSearch = ref("");

/* ---- accounts ------------------------------------------------------------ */

const accountsState = computed<LoadState<ServiceAccount[]>>(() => ({ status: "empty" }));
const activeAccountId = ref<string | null>(route.value.query.account ?? null);

/* Derived list states, typed so the screens' `LoadState` props stay exact. */
const recentsState = computed<LoadState<RepoSummaryModel[]>>(() =>
  recents.value.length > 0 ? { status: "data", data: recents.value } : { status: "empty" },
);
const nearbyState = computed<LoadState<RepoRoot[]>>(() => {
  switch (repositories.nearbyState.value) {
    case "scanning":
      return { status: "loading" };
    case "done":
      return { status: "data", data: repositories.nearby.value };
    default:
      return { status: "empty" };
  }
});

/**
 * Scan a folder for repositories.
 *
 * The user picks the folder rather than being scanned at $HOME: a home walk is
 * minutes on a large disk, and asking is one dialog instead of a silent,
 * unbounded filesystem crawl.
 */
async function scanNearby(): Promise<void> {
  const root = await pickDirectory();
  if (root) await repositories.discoverNearby(root);
}

/** Open a repository the scan found. It is not open yet, so a path, not an id. */
async function openNearbyPath(path: string): Promise<void> {
  const info = await repositories.open(path);
  if (info) navigate(repositoryPath(info.id));
}
const settingsState = computed<LoadState<SettingsDocument>>(() => ({
  status: "data",
  data: settingsDocument.value,
}));

/* ---- palettes ------------------------------------------------------------ */

const paletteMode = ref<"open" | "actions" | null>(null);
const paletteQuery = ref("");

function openPalette(mode: "open" | "actions"): void {
  paletteMode.value = mode;
  paletteQuery.value = "";
}

function closePalette(): void {
  paletteMode.value = null;
}

const paletteItems = computed<PaletteItemModel[]>(() => {
  if (paletteMode.value === "actions") {
    const hasRepo = selectedRepo.value !== null;
    return [
      { id: "action:new-branch", label: t("shell.toolbar.newBranch"), icon: "action.branch", group: t("shell.palette.groupBranches"), disabled: !hasRepo },
      { id: "action:checkout", label: t("shell.toolbar.checkoutBranch"), icon: "arrow.triangle.branch", group: t("shell.palette.groupBranches"), disabled: !hasRepo },
      { id: "action:refresh", label: t("shell.palette.refreshRepository"), icon: "action.refresh", group: t("shell.palette.groupRepository"), disabled: !hasRepo },
      { id: "action:settings", label: t("shell.palette.openSettings"), icon: "nav.settings", group: t("shell.palette.groupApplication") },
      { id: "action:account", label: t("shell.palette.openAccounts"), icon: "nav.services", group: t("shell.palette.groupApplication") },
      { id: "action:repos", label: t("shell.palette.repositoryList"), icon: "nav.repositories", group: t("shell.palette.groupApplication") },
    ];
  }

  const items: PaletteItemModel[] = [];
  for (const info of repositories.repositories.value) {
    items.push({
      id: `repo:${info.id}`,
      label: info.workdir ?? info.gitdir,
      detail: info.headBranch ?? undefined,
      icon: "shippingbox",
      group: t("shell.palette.groupRepositories"),
    });
  }
  for (const branch of localBranches.value) {
    items.push({
      id: `branch:${branch.fullName}`,
      label: branch.name,
      detail: branch.tipSummary,
      icon: branch.isHead ? "checkmark.circle.fill" : "arrow.triangle.branch",
      group: t("shell.palette.groupBranches"),
      disabled: selectedRepo.value === null,
    });
  }
  for (const tab of ["working-copy", "history", "branches", "compare"] as WorkspaceTab[]) {
    items.push({
      id: `tab:${tab}`,
      // The route table owns which key names a tab; capitalising the id is not
      // a translation, it is a coincidence that happened to work in English.
      label: t(WORKSPACE_TAB_LABEL_KEYS[tab]),
      icon: "nav.history",
      group: t("shell.palette.groupViews"),
      disabled: selectedRepo.value === null,
    });
  }
  return items;
});

const filteredPaletteItems = computed(() => {
  const needle = paletteQuery.value.trim().toLowerCase();
  if (needle === "") return paletteItems.value;
  return paletteItems.value.filter(
    (item) =>
      item.label.toLowerCase().includes(needle) ||
      (item.detail ?? "").toLowerCase().includes(needle),
  );
});

async function onPaletteSelect(id: string): Promise<void> {
  closePalette();
  if (id.startsWith("repo:")) return openRepository(id.slice(5));
  if (id.startsWith("branch:")) return checkoutFlow(id.slice(7));
  if (id.startsWith("tab:")) return goTab(id.slice(4) as WorkspaceTab);
  switch (id) {
    case "action:new-branch":
      return newBranchFlow();
    case "action:checkout":
      return checkoutFlow();
    case "action:refresh":
      await Promise.all([
        workingCopy.refresh(),
        branches.refresh(),
        history.refresh(),
      ]);
      return;
    case "action:settings":
      return void navigate(settingsPath());
    case "action:account":
      return void navigate(accountPath());
    case "action:repos":
      return void navigate(repositoriesPath());
    default:
      return;
  }
}

/* ---- dialogues ----------------------------------------------------------- */

function onDialogueResolve(result: DialogueResult): void {
  resolveDialogue(result);
}

function onDialogueCancel(id: string): void {
  cancelDialogue(id);
}

/* ---- shortcuts ----------------------------------------------------------- */

useKeyboardShortcuts([
  { key: "k", meta: true, run: () => openPalette("open") },
  { key: "p", meta: true, shift: true, run: () => openPalette("actions") },
  { key: ",", meta: true, run: () => navigate(settingsPath()) },
  { key: "r", meta: true, shift: true, run: () => void Promise.all([workingCopy.refresh(), branches.refresh(), history.refresh()]) },
  { key: "b", meta: true, shift: true, run: () => void newBranchFlow() },
]);

/* ---- misc ---------------------------------------------------------------- */

function onBlameRequest(path: string): void {
  const id = activeRepoId.value;
  if (!id) return;
  navigate(blamePath(id, path));
}

const blamePathFromQuery = computed(() => route.value.query.path ?? "");

const blameEmptyState = computed(() => ({ status: "empty" as const }));

watch(
  () => route.value.query.account,
  (value) => {
    activeAccountId.value = value ?? null;
  },
);
</script>

<template>
  <AppShell :title="windowTitle" :subtitle="windowSubtitle" :sidebar-width="sidebarWidth">
    <!-- The crash note from the previous run, until it is dismissed: after a
         crash the app relaunches looking normal, and the Diagnostics screen is
         the only place the note used to exist. -->
    <div v-if="crashNote" class="crash-banner" role="alert">
      <strong>{{ t("diagnostics.crash." + (crashNote.kind === "recovered_panic" ? "recoveredTitle" : crashNote.kind === "unclean_exit" ? "uncleanTitle" : "title")) }}</strong>
      <span class="crash-message">{{ crashNote.message }}</span>
      <button type="button" class="banner-verb" @click="openDiagnostics()">{{ t("diagnostics.open") }}</button>
      <button type="button" class="banner-verb" @click="onReportProblem(null)">{{ t("diagnostics.actions.report") }}</button>
      <button type="button" class="banner-verb" @click="onDismissCrash()">{{ t("diagnostics.crash.dismiss") }}</button>
    </div>

    <template #toolbar>
      <AppToolbar
        :verbs="toolbarVerbs"
        :activity="activitySummary"
        :debug-enabled="diagnosticsDebugEnabled"
        @open-diagnostics="navigate(diagnosticsPath())"
        @run="onToolbarRun"
        @choose="onToolbarChoose"
        @open-activity="navigate(activityPath())"
      />
    </template>

    <template #sidebar>
      <AppSidebar
        :current="summary"
        :recents="recents"
        :sections="sectionsWithCollapse"
        :selected-id="selectedSidebarId"
        :filter="sidebarFilter"
        :locked="false"
        @switch-repo="openRepository"
        @open-repositories="navigate(repositoriesPath())"
        @toggle-section="toggleSidebarSection($event)"
        @select-row="onSelectSidebarRow"
        @row-options="checkoutFlow($event.startsWith('branch:') ? $event.slice(7) : undefined)"
        @section-add="onSectionAdd($event)"
        @row-contextmenu="() => {}"
        @update:filter="sidebarFilter = $event"
      />
    </template>

    <!-- Repository list -->
    <RepositoryListScreen
      v-if="routeName === 'repositories'"
      :recents="recents"
      :recents-state="recentsState"
      :nearby="repositories.nearby.value"
      :nearby-state="nearbyState"
      :path-input="repositories.pathInput.value"
      :clone-url="repositories.cloneUrl.value"
      :filter="recentsFilter"
      :busy="repositories.busy.value"
      :error="repositories.lastError.value"
      :unavailable="null"
      @update:path-input="repositories.pathInput.value = $event"
      @update:clone-url="repositories.cloneUrl.value = $event"
      @update:filter="recentsFilter = $event"
      @browse="() => void repositories.browse()"
      @clone="(url) => void openAfter(repositories.clone(url))"
      @refresh-nearby="scanNearby"
      @open-nearby="openNearbyPath"
      @open="onOpenRequested"
      @init="(path) => void openAfter(repositories.init(path))"
      @open-recent="openRepository"
      @remove-recent="onRemoveRecent"
      @reveal-recent="onRevealRecent"
      @open-settings="navigate(settingsPath())"
    />

    <!-- Main repository window -->
    <RepositoryScreen
      v-else-if="routeName === 'repository'"
      :summary="summary"
      :tab="activeTab"
      :failed-tabs="failedTabs"
      @update:tab="goTab"
      @continue-operation="() => void runVerb(() => git.continueOperation(), t('shell.toast.operationContinued'))"
      @abort-operation="() => void runVerb(() => git.abortOperation(), t('shell.toast.operationAborted'))"
    >
      <template #working-copy>
        <WorkingCopyPane
          :state="workingCopy.state.value"
          :summary="workingCopy.summary.value"
          :selected-path="selectedPath"
          :merge="mergeStatus"
          :conflict-count="workingCopy.summary.value?.conflicted ?? 0"
          :detail-width="detailWidth"
          :selected-entry="selectedEntry"
          :diff="diff.document.value"
          :diff-mode="diffMode"
          :diff-options="diff.options.value"
          :subject="composerSubject"
          :body="composerBody"
          :amend="composerAmend"
          :sign-off="composerSignOff"
          :can-amend="selectedRepo?.headOid != null"
          :hooks-enabled="false"
          :hook-running="false"
          :hook-phase="null"
          :commit-busy="committing"
          :commit-options="commitOptions"
          :conflicted-only="conflictedOnly"
          @refresh="workingCopy.refresh()"
          @select-file="selectedPath = $event"
          @contextmenu="() => {}"
          @stage="(path) => void workingCopy.stage(path)"
          @unstage="(path) => void workingCopy.unstage(path)"
          @discard="(path) => void discardEntry(path)"
          @clear-assume-unchanged="(path) => void clearAssumeUnchanged(path)"
          @bulk-stage="(paths) => void workingCopy.stagePaths(paths)"
          @bulk-unstage="(paths) => void workingCopy.unstagePaths(paths)"
          @update:conflicted-only="conflictedOnly = $event"
          @update:detail-width="detailWidth = $event"
          @update:diff-mode="diffMode = $event"
          @update:diff-options="(options) => void diff.setOptions(options)"
          @hunk-action="onHunkAction"
          @diff-retry="() => void diff.reload()"
          @diff-show-anyway="() => void diff.reload()"
          @open-blame="onBlameRequest"
          @open-in-editor="(path) => void workingCopy.openInEditor(path)"
          @resolve-conflict="onResolveConflict"
          @resolve-merge="() => void runVerb(() => git.continueOperation(), t('shell.toast.mergeContinued'))"
          @abort-merge="() => void runVerb(() => git.abortOperation(), t('shell.toast.mergeAborted'))"
          @update:subject="composerSubject = $event"
          @update:body="composerBody = $event"
          @update:amend="composerAmend = $event"
          @update:sign-off="composerSignOff = $event"
          @commit="onCommit"
          @commit-choose="(optionId) => void onCommitChoose(optionId)"
        />
      </template>

      <template #history>
        <MasterDetail
          :detail-width="detailWidth"
          :min="320"
          :max="900"
          @update:detail-width="detailWidth = $event"
        >
          <template #master>
            <HistoryPane
              :state="history.state.value"
              :commits="history.commits.value"
              :layout="history.graph.value"
              :view="historyView"
              :search="historySearch"
              :selected-oid="history.selectedOid.value"
              :head-oid="selectedRepo?.headOid ?? null"
              :truncated="history.truncated.value"
              :search-active="historySearch.text.trim() !== ''"
              :type-filter-unavailable="historyTypeUnavailable"
              @select="onHistorySelect"
              @activate="onHistorySelect"
              @need-more="history.loadMore()"
              @ref-click="onHistoryRefClick"
              @update:view="historyView = $event"
              @update:search="historySearch = $event"
              @refresh="history.refresh()"
            />
          </template>
          <template #detail>
            <CommitDetailPane
              :state="history.detail.value"
              :changeset="history.changeset.value"
              :diff="null"
              :diff-mode="diffMode"
              :diff-options="diff.options.value"
              :selected-path="history.selectedPath.value"
              :detail-width="detailWidth"
              @copy-hash="copyText"
              @copy-summary="copyText"
              @select-file="(path) => history.selectPath(path)"
              @file-contextmenu="() => {}"
              @open-blame="onBlameRequest"
              @open-in-editor="(path) => void workingCopy.openInEditor(path)"
              @browse-tree="goTab('working-copy')"
              @select-parent="(oid) => void history.select(oid)"
              @update:detail-width="detailWidth = $event"
              @update:diff-mode="diffMode = $event"
              @update:diff-options="(options) => void diff.setOptions(options)"
              @hunk-action="() => {}"
              @diff-retry="() => void diff.reload()"
              @diff-show-anyway="() => void diff.reload()"
            />
          </template>
        </MasterDetail>
      </template>

      <template #branches>
        <MasterDetail
          :detail-width="detailWidth"
          :min="320"
          :max="820"
          @update:detail-width="detailWidth = $event"
        >
          <template #master>
            <BranchListPane
              :state="branches.state.value"
              :branches="branches.allBranches.value"
              :selected-name="selectedBranchName"
              :filter="branchFilter"
              :search="branchSearch"
              :current-branch="selectedRepo?.headBranch ?? null"
              @select="selectedBranchName = $event"
              @activate="checkoutFlow($event)"
              @contextmenu="() => {}"
              @create="newBranchFlow()"
              @checkout="checkoutFlow($event)"
              @merge="(name) => void mergeBranch(name)"
              @rebase="(name) => void rebaseBranch(name)"
              @delete="(name) => void deleteBranch(name)"
              @rename="(name) => void renameBranchFlow(name)"
              @update:filter="branchFilter = $event"
              @update:search="branchSearch = $event"
              @refresh="branches.refresh()"
            />
          </template>
          <template #detail>
            <BranchDetailPane
              :branch="selectedBranchInfo"
              :current-branch="selectedRepo?.headBranch ?? null"
              :can-delete="true"
              @checkout="selectedBranchInfo && checkoutFlow(selectedBranchInfo.fullName)"
              @merge-into-current="() => void mergeBranch(selectedBranchInfo?.name ?? selectedBranchName)"
              @rebase-onto="() => void rebaseBranch(selectedBranchInfo?.name ?? selectedBranchName)"
              @push="() => void pushSelectedBranch()"
              @push-choose="() => void pushSelectedBranch()"
              @pull="() => void checkoutSelectedBranch()"
              @rename="() => void renameBranchFlow(selectedBranchInfo?.name ?? selectedBranchName)"
              @remove="() => void deleteBranch(selectedBranchInfo?.name ?? selectedBranchName)"
              @show-in-history="goTab('history')"
              @open-compare="goTab('compare')"
              @copy-name="() => void copyText(selectedBranchInfo?.name ?? '')"
            />
          </template>
        </MasterDetail>
      </template>

      <template #compare>
        <ComparePane
          :state="compare.state.value"
          :comparison="compare.comparison.value"
          :left="compare.left.value"
          :right="compare.right.value"
          :selected-oid="history.selectedOid.value"
          :detail-width="detailWidth"
          :busy="compare.busy.value"
          :unavailable="null"
          @choose-ref="pickCompareRef"
          @swap="compare.swap()"
          @select-commit="history.select($event)"
          @open-commit="onOpenCommitFromCompare"
          @update:detail-width="detailWidth = $event"
          @refresh="() => void compare.reload()"
        >
          <template #detail>
            <CommitDetailPane
              :state="history.detail.value"
              :changeset="history.changeset.value"
              :diff="null"
              :diff-mode="diffMode"
              :diff-options="diff.options.value"
              :selected-path="history.selectedPath.value"
              :detail-width="detailWidth"
              @copy-hash="copyText"
              @copy-summary="copyText"
              @select-file="(path) => history.selectPath(path)"
              @file-contextmenu="() => {}"
              @open-blame="onBlameRequest"
              @open-in-editor="(path) => void workingCopy.openInEditor(path)"
              @browse-tree="goTab('history')"
              @select-parent="(oid) => void history.select(oid)"
              @update:detail-width="detailWidth = $event"
              @update:diff-mode="diffMode = $event"
              @update:diff-options="(options) => void diff.setOptions(options)"
              @hunk-action="() => {}"
              @diff-retry="() => void history.refresh()"
              @diff-show-anyway="() => void history.refresh()"
            />
          </template>
        </ComparePane>
      </template>
    </RepositoryScreen>

    <!-- Blame -->
    <BlamePane
      v-else-if="routeName === 'blame'"
      :state="blameEmptyState"
      :blame="null"
      :path="blamePathFromQuery"
      :popover="null"
      @annotate="() => {}"
      @select-line="() => {}"
      @close-popover="() => {}"
      @open-commit="() => {}"
      @copy-oid="(oid) => void copyText(oid)"
      @open-in-editor="() => blamePathFromQuery && void workingCopy.openInEditor(blamePathFromQuery)"
      @open-diff="goTab('working-copy')"
      @follow-rename="() => {}"
      @retry="() => {}"
    />

    <!-- Settings -->
    <SettingsScreen
      v-else-if="routeName === 'settings'"
      :document="settingsState"
      :active-tab="settingsTab"
      :theme-mode="shell.themeMode.value"
      :locale="locale"
      :search="settingsSearch"
      @update:active-tab="navigate(settingsPath($event))"
      @update:theme-mode="(mode) => setSetting('shell.appearance', mode)"
      @update:locale="changeLocale"
      @update:search="settingsSearch = $event"
      @set="setSetting"
      @reset="resetSetting"
      @choose-path="onChooseSettingPath"
      @refresh="() => {}"
    />

    <!-- About / updates / release notes / feedback -->
    <ActivityScreen
      v-else-if="routeName === 'activity'"
      :entries="activity.entries.value"
      :running="activity.running.value.length"
      @clear="activity.clearFinished()"
    />

    <DiagnosticsScreen
      v-else-if="routeName === 'diagnostics'"
      v-bind="diagnosticsState"
      :call-for="diagnostics.callFor"
      @refresh="diagnostics.refresh"
      @set-debug="diagnostics.setDebug"
      @reveal-logs="diagnostics.revealLogs"
      @dismiss-crash="onDismissCrash"
      @copy-details="diagnostics.copyFailureDetails"
      @copy-report="diagnostics.copyReport"
      @export-report="onExportReport"
      @open-devtools="diagnostics.openDevtools"
      @relaunch="restartToUpdate"
      @report-problem="onReportProblem"
    />

    <AboutScreen
      v-else-if="routeName === 'about'"
      :info="appInfo"
      :check="appCheck"
      :notes="appNotes"
      :install="appInstall"
      :install-progress="installProgress"
      :auto-check="autoCheckUpdates"
      :last-checked-at="lastCheckedAt"
      :locale="locale"
      :feedback-busy="feedbackBusy"
      :feedback-failed="feedbackFailed"
      @check="() => void app.checkForUpdates()"
      @install="() => void app.installUpdate()"
      @restart="() => void restartToUpdate()"
      @copy-diagnostics="() => void copyText(appInfoDiagnostics())"
      @open-diagnostics="navigate(diagnosticsPath())"
      @retry-notes="() => void app.loadReleaseNotes(locale)"
      @notes-locale="(next: Locale) => void app.loadReleaseNotes(next)"
      @open-link="(url: string) => void openExternalLink(url)"
      @feedback="(kind: FeedbackKind, title: string) => void onFeedback(kind, title)"
      @update:locale="changeLocale"
      @update:auto-check="app.setAutoCheck"
    />

    <!-- Accounts -->
    <AccountScreen
      v-else
      :accounts="accountsState"
      :selected-id="activeAccountId"
      :unavailable="t('shell.unavailable.accounts')"
      @select="activeAccountId = $event"
      @refresh="() => {}"
    />

    <template #overlay>
      <ToastHost :toasts="toastViews" @dismiss="onToastDismiss" @action="onToastAction" />

      <DialogueHost
        :request="shell.activeDialogue.value"
        :busy="false"
        @resolve="onDialogueResolve"
        @cancel="onDialogueCancel"
      />

      <CommandPalette
        :open="paletteMode !== null"
        :query="paletteQuery"
        :items="filteredPaletteItems"
        :heading="paletteMode === 'actions' ? t('shell.palette.headingActions') : t('shell.palette.headingOpen')"
        :placeholder="
          paletteMode === 'actions'
            ? t('shell.palette.placeholderActions')
            : t('shell.palette.placeholderOpen')
        "
        :empty-text="t('shell.palette.empty')"
        @update:query="paletteQuery = $event"
        @select="onPaletteSelect"
        @close="closePalette"
      />
    </template>
  </AppShell>
</template>

<style scoped>
.crash-banner {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-2) var(--md-space-3);
  border-bottom: 1px solid var(--md-danger);
  background: var(--md-bg-danger-soft);
  color: var(--md-fg);
  font-size: var(--md-text-sm);
}

.crash-banner strong {
  flex: none;
}

.crash-banner .crash-message {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--md-fg-muted);
}

.crash-banner .banner-verb {
  border: none;
  background: transparent;
  color: var(--md-danger);
  font-size: var(--md-text-xs);
  font-weight: 600;
  padding: 0;
}

.crash-banner .banner-verb:hover {
  text-decoration: underline;
}
</style>
