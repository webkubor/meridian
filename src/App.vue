<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";

import type { BranchInfo, CommitInfo, EntryStatus, ErrorPayload, DiffPerspective, FeedbackKind, HistoryCommitsOptions } from "./bridges/bindings";
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
import { useAppEvents } from "./composables/useAppEvents";
import { useApp } from "./composables/useApp";
import { useBranches } from "./composables/useBranches";
import { useDiff } from "./composables/useDiff";
import { useGit } from "./composables/useGit";
import { useHistory } from "./composables/useHistory";
import { useRepositories } from "./composables/useRepositories";
import { useWorkingCopy } from "./composables/useWorkingCopy";
import AboutScreen from "./components/screens/AboutScreen.vue";
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
  blamePath,
  isWorkspaceTab,
  repositoriesPath,
  repositoryPath,
  settingsPath,
  type WorkspaceTab,
} from "./router/routes";
import { navigate, useRoute } from "./router";
import { currentLocale, setLocale, type Locale } from "./i18n";
import { openExternal } from "./bridges/external";
import { relaunch } from "./bridges/commands";
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
const workingCopy = useWorkingCopy();
const branches = useBranches();
const history = useHistory();
const diff = useDiff();
const git = useGit();

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

async function restartToUpdate(): Promise<void> {
  // macOS installs the new bundle beside the running one; the process has to
  // come back up to be running it. Nothing else about this session survives,
  // which is why the button is behind an explicit "Ready" state.
  await relaunch();
}

async function openExternalLink(url: string): Promise<void> {
  if (!(await openExternal(url))) {
    shell.pushToast({ kind: "danger", text: "Could not open that link." });
  }
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
    text: `Opened GitHub with ${target.prefilled.length} detail${target.prefilled.length === 1 ? "" : "s"} filled in.`,
  });
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
  if (routeName.value === "settings") return "Settings";
  if (routeName.value === "account") return "Accounts";
  if (routeName.value === "about") return "About Meridian";
  if (routeName.value === "repositories") return "Repositories";
  const head = selectedRepo.value?.headBranch;
  return head ? `On ${head}` : selectedRepo.value?.gitdir ?? null;
});

/* ---- sidebar ------------------------------------------------------------- */

const sidebarWidth = usePersistedNumber("meridian.sidebarWidth", 216, 180, 340);
const sidebarFilter = ref("");
const selectedSidebarId = ref<string | null>(null);

const localBranches = computed<BranchInfo[]>(() =>
  branches.branches.value.filter((branch) => !branch.isRemote),
);

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
      title: "Working copy",
      icon: "nav.workingCopy",
      pseudo: true,
      collapsed: false,
      emptyCaption: "No local changes",
      rows: [
        {
          id: "sidebar:working-copy",
          label: "Local changes",
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
      title: "History",
      icon: "nav.history",
      collapsed: false,
      emptyCaption: "No commits",
      rows:
        history.commits.value.length > 0 && keep("history")
          ? [
              {
                id: "sidebar:history",
                label: "All commits",
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
      title: "Branches",
      icon: "nav.branch",
      collapsed: false,
      emptyCaption: "No branches",
      rows: branchRows,
    },
    {
      id: "remotes",
      title: "Remotes",
      icon: "cloud",
      collapsed: true,
      emptyCaption: "No remote branches",
      rows: remoteRows,
    },
    {
      id: "tags",
      title: "Tags",
      icon: "nav.tags",
      collapsed: true,
      emptyCaption: "No tags",
      rows: [],
    },
    {
      id: "stashes",
      title: "Stashes",
      icon: "nav.stashes",
      collapsed: true,
      emptyCaption: "No stashes",
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
  void runVerb(() => repositories.reveal(id), "Revealed in the file manager.");
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

function openRepository(id: string): void {
  repositories.select(id);
  navigate(repositoryPath(id));
}

function onOpenRequested(path: string): void {
  void repositories.open(path);
}

/* ---- shell helpers ------------------------------------------------------- */

function reportError(error: unknown, fallback: string): void {
  const message =
    error && typeof error === "object" && "message" in error
      ? String((error as { message: unknown }).message)
      : fallback;
  shell.pushToast({ kind: "danger", text: message });
}

/** Copy text (hashes, branch names, keys) to the clipboard. */
async function copyText(text: string): Promise<void> {
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    shell.pushToast({ kind: "success", text: "Copied to the clipboard." });
  } catch {
    shell.pushToast({ kind: "danger", text: "Could not copy to the clipboard." });
  }
}

/** Run a repo-level git verb, surfacing the engine's own error or a success note. */
async function runVerb(action: () => Promise<ErrorPayload | null>, success: string): Promise<void> {
  const error = await action();
  if (error) reportError(error, "The action failed.");
  else shell.pushToast({ kind: "success", text: success });
}

/* ---- toolbar ------------------------------------------------------------- */

const toolbarVerbs = computed<ToolbarVerbModel[]>(() => {
  const hasRepo = selectedRepo.value !== null;
  const hasRemote = repositories.defaultRemote.value !== null;
  const hasHead = selectedRepo.value?.headBranch != null;

  const repoReason = hasRepo ? undefined : "Open a repository first.";
  const remoteReason = !hasRepo
    ? "Open a repository first."
    : hasRemote
      ? undefined
      : "This repository has no remote configured.";
  const pushReason = !hasRepo
    ? "Open a repository first."
    : !hasRemote
      ? "This repository has no remote configured."
      : !hasHead
        ? "HEAD is detached — check out a branch to push."
        : undefined;

  const branchOptions: SplitActionOption[] = [
    { id: "branch.new", label: "New branch…", disabled: !hasRepo },
    { id: "branch.checkout", label: "Check out branch…", disabled: !hasRepo },
    { id: "branch.rename", label: "Rename branch…", disabled: !hasRepo },
  ];

  return [
    {
      id: "branch",
      label: "Branch",
      icon: "action.branch",
      group: "global",
      disabled: !hasRepo,
      reason: repoReason,
      options: branchOptions,
    },
    { id: "fetch", label: "Fetch", icon: "action.fetch", group: "global", disabled: remoteReason !== undefined, reason: remoteReason },
    { id: "pull", label: "Pull", icon: "action.pull", group: "global", disabled: remoteReason !== undefined, reason: remoteReason },
    { id: "push", label: "Push", icon: "action.push", group: "global", disabled: pushReason !== undefined, reason: pushReason },
    { id: "stash", label: "Stash", icon: "action.stash", group: "global", disabled: !hasRepo, reason: repoReason },

    { id: "quickOpen", label: "Quick open", icon: "action.quickOpen", group: "utility", disabled: false },
    { id: "quickActions", label: "Quick actions", icon: "action.quickActions", group: "utility", disabled: false },
    { id: "settings", label: "Settings", icon: "nav.settings", group: "utility", disabled: false },
    { id: "account", label: "Accounts", icon: "nav.services", group: "utility", disabled: false },
    { id: "about", label: "About Meridian", icon: "info.circle", group: "utility", disabled: false },
  ];
});

const activitySummary = computed(() => ({
  count: 0,
  busy: false,
  stalled: false,
  unavailable: true,
  unavailableReason: "Activity tracking is not available in this build.",
}));

async function onToolbarRun(id: string): Promise<void> {
  switch (id) {
    case "branch":
      await newBranchFlow();
      return;
    case "fetch":
      await runVerb(() => git.fetch(), "Fetched.");
      return;
    case "pull":
      await runVerb(() => git.pull(), "Pulled.");
      return;
    case "push":
      await runVerb(() => git.push(), "Pushed.");
      return;
    case "stash":
      await runVerb(() => git.stash(), "Stashed the working copy.");
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
    case "about":
      navigate(aboutPath());
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
    title: "New branch",
    label: "Branch name",
    value: "",
    placeholder: "feature/short-description",
    helpText: "The branch starts at the current HEAD.",
    validate: "refname",
    option: { label: "Check out the new branch", checked: true },
  };
  const result = await askDialogue(request);
  if (!result || result.value.trim() === "") return;
  const created = await branches.create(result.value.trim());
  if (created) {
    shell.pushToast({ kind: "success", text: `Created branch ${created.name}.` });
  } else if (branches.lastError.value) {
    reportError(branches.lastError.value, "Could not create the branch.");
  }
}

/** What the sidebar's per-section "+" adds, or the noun to name when it cannot. */
const SECTION_ADD_NOUN: Record<string, string> = {
  "working-copy": "working-copy entries",
  history: "commits",
  remotes: "remotes",
  tags: "tags",
  stashes: "stashes",
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
    text: `Adding ${SECTION_ADD_NOUN[sectionId] ?? "entries"} is not available in this build.`,
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
    title: "Rename branch",
    label: "New name",
    value: oldName,
    placeholder: "feature/renamed",
    validate: "refname",
  });
  if (!result || result.value.trim() === "" || result.value.trim() === oldName) return;

  const renamed = await branches.rename(oldName, result.value.trim());
  if (renamed) {
    selectedBranchName.value = renamed.fullName;
    shell.pushToast({ kind: "success", text: `Renamed to ${renamed.name}.` });
  } else if (branches.lastError.value) {
    reportError(branches.lastError.value, "Could not rename the branch.");
  }
}

/** Merge `name` into the current branch (docs/03 §12 `merge_start`). */
async function mergeBranch(name: string | null): Promise<void> {
  if (!name) return;
  await runVerb(() => git.merge(name), `Merged ${name}.`);
}

/** Rebase the current branch onto `name` (docs/03 §12 `rebase_start`). */
async function rebaseBranch(name: string | null): Promise<void> {
  if (!name) return;
  await runVerb(() => git.rebase(name), `Rebased onto ${name}.`);
}

/** Delete a branch behind an explicit confirmation (docs/03 §4 `branch_delete`). */
async function deleteBranch(name: string | null): Promise<void> {
  if (!name) return;
  const branch = branches.branches.value.find((entry) => entry.name === name || entry.fullName === name);
  const label = branch?.name ?? name;
  const confirmed = await askDialogue({
    kind: "confirm",
    id: dialogueId(),
    title: "Delete branch?",
    message: `Delete ${label}? This cannot be undone.`,
    items: [label],
    confirmLabel: "Delete branch",
    danger: true,
  });
  if (!confirmed) return;

  const ok = await branches.remove(branch?.name ?? name);
  if (ok) shell.pushToast({ kind: "success", text: `Deleted ${label}.` });
  else if (branches.lastError.value) reportError(branches.lastError.value, "Could not delete the branch.");
}

async function checkoutFlow(preset?: string): Promise<void> {
  const name =
    preset ??
    (await askDialogue({
      kind: "chooseBranch",
      id: dialogueId(),
      title: "Check out branch",
      branches: branches.branches.value,
      current: selectedRepo.value?.headBranch ?? null,
    }))?.value;
  if (!name) return;

  const dirty = workingCopyTotal.value > 0;
  if (dirty) {
    const confirmed = await askDialogue({
      kind: "confirm",
      id: dialogueId(),
      title: "Check out with local changes?",
      message: `You have ${workingCopyTotal.value} uncommitted change(s). Checking out may fail or carry them over.`,
      items: workingCopy.entries.value.slice(0, 8).map((entry) => entry.path),
      confirmLabel: "Check out anyway",
    });
    if (!confirmed) return;
  }

  const ok = await branches.checkout(name);
  if (ok) shell.pushToast({ kind: "success", text: `Checked out ${name}.` });
  else if (branches.lastError.value) reportError(branches.lastError.value, "Could not check out the branch.");
}

async function discardEntry(path: string): Promise<void> {
  const confirmed = await askDialogue({
    kind: "confirm",
    id: dialogueId(),
    title: "Discard changes?",
    message: "This permanently discards the changes in this file. It cannot be undone.",
    items: [path],
    confirmLabel: "Discard changes",
    danger: true,
  });
  if (!confirmed) return;
  const ok = await workingCopy.discard(path);
  if (ok) shell.pushToast({ kind: "success", text: "Discarded the file's changes." });
  else reportError(workingCopy.lastError.value, "Could not discard the changes.");
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

const commitOptions: SplitActionOption[] = [{ id: "commit.push", label: "Commit & Push" }];

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
    shell.pushToast({ kind: "warning", text: "Write a commit message first." });
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
    shell.pushToast({ kind: "success", text: "Committed." });
    await history.refresh();
  } else {
    reportError(workingCopy.lastError.value, "Could not create the commit.");
  }
  return ok;
}

function onCommit(): void {
  void commitNow();
}

async function onCommitChoose(optionId: string): Promise<void> {
  if (optionId === "commit.push") {
    if (await commitNow()) await runVerb(() => git.push(), "Committed and pushed.");
  }
}

function onResolveConflict(path: string): void {
  void runVerb(() => git.resolveConflict([path]), "Conflict resolved.");
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
const historyTypeUnavailable = "Filtering by merge status is not available in this build.";

/* ---- branches ------------------------------------------------------------ */

const selectedBranchName = ref<string | null>(null);
const branchFilter = ref<"local" | "remote" | "all">("local");
const branchSearch = ref("");

const selectedBranchInfo = computed(() => {
  const name = selectedBranchName.value;
  if (!name) return null;
  const branch = branches.branches.value.find((entry) => entry.fullName === name) ?? null;
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
  window.localStorage.setItem(SETTINGS_KEY, JSON.stringify(storedSettings.value));
  if (key === "shell.appearance") shell.setThemeMode(value as "auto" | "light" | "dark");
}

function resetSetting(key: string): void {
  const next = { ...storedSettings.value };
  delete next[key];
  storedSettings.value = next;
  window.localStorage.setItem(SETTINGS_KEY, JSON.stringify(next));
}

const settingsDocument = computed<SettingsDocument>(() => ({
  version: 1,
  tabs: [
    {
      id: "appearance",
      label: "Appearance",
      icon: "sun.max",
      source: "shell",
      groups: [
        {
          id: "editor",
          label: "Editor",
          rows: [
            {
              key: "shell.editorFontSize",
              label: "Editor font size",
              kind: "int",
              value: editorFontSize.value,
              defaultValue: 13,
              min: 10,
              max: 22,
              help: "Applies to diff and blame views.",
              shellOwned: true,
            },
          ],
        },
      ],
    },
    {
      id: "git",
      label: "Git",
      icon: "action.branch",
      source: "shell",
      groups: [
        {
          id: "safety",
          label: "Safety",
          rows: [
            {
              key: "shell.confirmCheckout",
              label: "Confirm before checking out with local changes",
              kind: "bool",
              value: confirmCheckout.value,
              defaultValue: true,
              help: "Shows a confirmation whenever the working copy is dirty.",
              shellOwned: true,
            },
          ],
        },
      ],
    },
    {
      id: "general",
      label: "General",
      icon: "gearshape",
      source: "schema",
      groups: [],
    },
    {
      id: "services",
      label: "Services",
      icon: "cloud",
      source: "schema",
      groups: [],
    },
    {
      id: "advanced",
      label: "Advanced",
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
const nearbyState = computed<LoadState<RepoSummaryModel[]>>(() => ({ status: "empty" }));
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
      { id: "action:new-branch", label: "New branch…", icon: "action.branch", group: "Branches", disabled: !hasRepo },
      { id: "action:checkout", label: "Check out branch…", icon: "arrow.triangle.branch", group: "Branches", disabled: !hasRepo },
      { id: "action:refresh", label: "Refresh repository", icon: "action.refresh", group: "Repository", disabled: !hasRepo },
      { id: "action:settings", label: "Open settings", icon: "nav.settings", group: "Application" },
      { id: "action:account", label: "Open accounts", icon: "nav.services", group: "Application" },
      { id: "action:repos", label: "Repository list", icon: "nav.repositories", group: "Application" },
    ];
  }

  const items: PaletteItemModel[] = [];
  for (const info of repositories.repositories.value) {
    items.push({
      id: `repo:${info.id}`,
      label: info.workdir ?? info.gitdir,
      detail: info.headBranch ?? undefined,
      icon: "shippingbox",
      group: "Repositories",
    });
  }
  for (const branch of localBranches.value) {
    items.push({
      id: `branch:${branch.fullName}`,
      label: branch.name,
      detail: branch.tipSummary,
      icon: branch.isHead ? "checkmark.circle.fill" : "arrow.triangle.branch",
      group: "Branches",
      disabled: selectedRepo.value === null,
    });
  }
  for (const tab of ["working-copy", "history", "branches", "compare"] as WorkspaceTab[]) {
    items.push({
      id: `tab:${tab}`,
      label: tab === "working-copy" ? "Working copy" : tab.charAt(0).toUpperCase() + tab.slice(1),
      icon: "nav.history",
      group: "Views",
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

const compareEmptyState = computed(() => ({ status: "empty" as const }));
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
    <template #toolbar>
      <AppToolbar
        :verbs="toolbarVerbs"
        :activity="activitySummary"
        @run="onToolbarRun"
        @choose="onToolbarChoose"
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
      :nearby="[]"
      :nearby-state="nearbyState"
      :path-input="repositories.pathInput.value"
      :filter="recentsFilter"
      :busy="repositories.busy.value"
      :error-message="repositories.lastError.value?.message ?? null"
      :error-detail="repositories.lastError.value?.detail ?? null"
      :unavailable="{
        browse: 'Choosing a folder is not available in this build.',
        clone: 'Cloning is not available in this build.',
        nearby: 'Scanning for repositories is not available in this build.',
      }"
      @update:path-input="repositories.pathInput.value = $event"
      @update:filter="recentsFilter = $event"
      @open="onOpenRequested"
      @init="(path) => void repositories.init(path)"
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
      @update:tab="goTab"
      @continue-operation="() => void runVerb(() => git.continueOperation(), 'Operation continued.')"
      @abort-operation="() => void runVerb(() => git.abortOperation(), 'Operation aborted.')"
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
          @bulk-stage="(paths) => paths.forEach((path) => void workingCopy.stage(path))"
          @bulk-unstage="(paths) => paths.forEach((path) => void workingCopy.unstage(path))"
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
          @resolve-merge="() => void runVerb(() => git.continueOperation(), 'Merge continued.')"
          @abort-merge="() => void runVerb(() => git.abortOperation(), 'Merge aborted.')"
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
              :branches="branches.branches.value"
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
              @push="() => void runVerb(() => git.push(), 'Pushed.')"
              @push-choose="() => void runVerb(() => git.push(), 'Pushed.')"
              @pull="() => void runVerb(() => git.pull(), 'Pulled.')"
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
          :state="compareEmptyState"
          :comparison="null"
          :left="null"
          :right="null"
          :detail-width="detailWidth"
          unavailable="Comparing revisions is not available in this build."
          @select-commit="() => {}"
          @open-commit="() => {}"
          @update:detail-width="detailWidth = $event"
          @refresh="() => {}"
        />
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
      @refresh="() => {}"
    />

    <!-- About / updates / release notes / feedback -->
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
      unavailable="Connecting and managing hosting accounts is not available in this build."
      @select="activeAccountId = $event"
      @refresh="() => {}"
    />

    <template #overlay>
      <ToastHost :toasts="shell.toasts.value" @dismiss="shell.dismissToast" @action="shell.dismissToast" />

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
        :heading="paletteMode === 'actions' ? 'Quick actions' : 'Quick open'"
        :placeholder="paletteMode === 'actions' ? 'Run a command' : 'Search repositories, branches and views'"
        empty-text="No matches"
        @update:query="paletteQuery = $event"
        @select="onPaletteSelect"
        @close="closePalette"
      />
    </template>
  </AppShell>
</template>
