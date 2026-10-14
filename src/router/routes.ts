/**
 * Route table (docs/04 §5). The app is a single window with a fixed set of
 * screens; routing is a hash so the Tauri window keeps one document.
 *
 * Params: `repoId` is the canonicalised gitdir (RepoId) — always encoded, it
 * contains slashes. `tab` / `section` / `path` travel as query values.
 */

export type RouteName = "repositories" | "repository" | "blame" | "settings" | "account" | "about";

export interface RouteDefinition {
  name: RouteName;
  /** Pattern with `:param` segments. */
  path: string;
}

export const ROUTE_DEFINITIONS: RouteDefinition[] = [
  { name: "repositories", path: "/repos" },
  { name: "repository", path: "/repo/:repoId" },
  { name: "blame", path: "/repo/:repoId/blame" },
  { name: "settings", path: "/settings" },
  { name: "account", path: "/account" },
  { name: "about", path: "/about" },
];

/** The workspace tabs of the repository screen (docs/04 §5, S6/S8/S9). */
export type WorkspaceTab = "working-copy" | "history" | "branches" | "compare";

export const WORKSPACE_TABS: { id: WorkspaceTab; label: string; icon: string }[] = [
  { id: "working-copy", label: "Working copy", icon: "doc.text" },
  { id: "history", label: "History", icon: "clock" },
  { id: "branches", label: "Branches", icon: "arrow.triangle.branch" },
  { id: "compare", label: "Compare", icon: "arrow.left.arrow.right" },
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

export function settingsPath(section?: string): string {
  return section ? `/settings?section=${encodeURIComponent(section)}` : "/settings";
}

export function accountPath(accountId?: string): string {
  return accountId ? `/account?account=${encodeURIComponent(accountId)}` : "/account";
}

export function aboutPath(): string {
  return "/about";
}

export function isWorkspaceTab(value: string | undefined): value is WorkspaceTab {
  return value === "working-copy" || value === "history" || value === "branches" || value === "compare";
}
