/**
 * Route table (docs/04 §5). The app is a single window with a fixed set of
 * screens; routing is a hash so the Tauri window keeps one document.
 *
 * Params: `repoId` is the canonicalised gitdir (RepoId) — always encoded, it
 * contains slashes. `tab` / `section` / `path` travel as query values.
 *
 * Names here are identifiers, not copy: `aboutPath()` returns the string
 * `"/about"`, not the words "About Meridian". Display text lives in the
 * catalogue, and a route entry names its catalogue entry through `labelKey`.
 */

export type RouteName =
  | "repositories"
  | "repository"
  | "blame"
  | "activity"
  | "settings"
  | "account"
  | "about"
  | "diagnostics";

export interface RouteDefinition {
  name: RouteName;
  /** Pattern with `:param` segments. */
  path: string;
}

export const ROUTE_DEFINITIONS: RouteDefinition[] = [
  { name: "repositories", path: "/repos" },
  { name: "repository", path: "/repo/:repoId" },
  { name: "blame", path: "/repo/:repoId/blame" },
  { name: "activity", path: "/activity" },
  { name: "settings", path: "/settings" },
  { name: "account", path: "/account" },
  { name: "about", path: "/about" },
  { name: "diagnostics", path: "/diagnostics" },
];

/** The workspace tabs of the repository screen (docs/04 §5, S6/S8/S9). */
export type WorkspaceTab = "working-copy" | "history" | "branches" | "compare";

/**
 * Tab id → catalogue key. `App.vue`'s command switch labels its entries from
 * this map, so a tab has exactly one name in the app.
 */
export const WORKSPACE_TAB_LABEL_KEYS: Record<WorkspaceTab, string> = {
  "working-copy": "shell.tab.workingCopy",
  history: "shell.tab.history",
  branches: "shell.tab.branches",
  compare: "shell.tab.compare",
};

/**
 * `label` is still here because `screens/RepositoryScreen.vue` renders it and
 * that file is not part of this change; `labelKey` is the catalogue source of
 * truth. The English `label` goes when its owner renders `$t(entry.labelKey)`.
 */
export const WORKSPACE_TABS: { id: WorkspaceTab; label: string; labelKey: string; icon: string }[] = [
  { id: "working-copy", label: "Working copy", labelKey: WORKSPACE_TAB_LABEL_KEYS["working-copy"], icon: "doc.text" },
  { id: "history", label: "History", labelKey: WORKSPACE_TAB_LABEL_KEYS.history, icon: "clock" },
  { id: "branches", label: "Branches", labelKey: WORKSPACE_TAB_LABEL_KEYS.branches, icon: "arrow.triangle.branch" },
  { id: "compare", label: "Compare", labelKey: WORKSPACE_TAB_LABEL_KEYS.compare, icon: "arrow.left.arrow.right" },
];

export function repositoriesPath(): string {
  return "/repos";
}

export function repositoryPath(repoId: string, tab?: WorkspaceTab): string {
  const base = `/repo/${encodeURIComponent(repoId)}`;
  return tab ? `${base}?tab=${tab}` : base;
}

export function blamePath(repoId: string, filePath: string): string {
  return `/repo/${encodeURIComponent(repoId)}/blame?path=${encodeURIComponent(filePath)}`;
}

export function activityPath(): string {
  return "/activity";
}

export function settingsPath(section?: string): string {
  return section ? `/settings?section=${encodeURIComponent(section)}` : "/settings";
}

export function accountPath(accountId?: string): string {
  return accountId ? `/account?account=${encodeURIComponent(accountId)}` : "/account";
}

export function aboutPath(): string {
  return "/about";
}

export function diagnosticsPath(): string {
  return "/diagnostics";
}

export function isWorkspaceTab(value: string | undefined): value is WorkspaceTab {
  return value === "working-copy" || value === "history" || value === "branches" || value === "compare";
}
