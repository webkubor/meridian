import { computed, ref } from "vue";
import { defineStore } from "pinia";

import type {
  ErrorPayload,
  RepoId,
  RepoInfo,
  RepoRoot,
  RepositoryRemote,
} from "../bridges/bindings";
import * as commands from "../bridges/commands";
import { markHandled } from "../diagnostics/failures";

/**
 * Open repositories and the current selection. One store per domain module
 * (docs/02 §13); the store holds domain types only and issues commands through
 * `bridges/commands`.
 */
export const useReposStore = defineStore("repos", () => {
  const repositories = ref<RepoInfo[]>([]);
  const selectedId = ref<RepoId | null>(null);
  const busy = ref(false);
  const lastError = ref<ErrorPayload | null>(null);
  /** Remotes of the selected repository (`repository_remotes_load`). */
  const remotes = ref<RepositoryRemote[]>([]);
  const remotesLoading = ref(false);

  const selected = computed(
    () => repositories.value.find((repo) => repo.id === selectedId.value) ?? null,
  );

  const defaultRemote = computed<string | null>(() => {
    const origin = remotes.value.find((remote) => remote.name === "origin");
    return origin?.name ?? remotes.value[0]?.name ?? null;
  });

  function upsert(info: RepoInfo): void {
    const index = repositories.value.findIndex((repo) => repo.id === info.id);
    if (index >= 0) {
      repositories.value[index] = info;
    } else {
      repositories.value.push(info);
    }
  }

  /** Re-read HEAD after a checkout, merge, rebase or pull moves it. */
  async function refreshInfo(repo: RepoId): Promise<RepoInfo | null> {
    try {
      const info = await commands.repositoryRefresh(repo);
      upsert(info);
      return info;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      markHandled(error);
      return null;
    }
  }

  async function loadRemotes(repo: RepoId): Promise<void> {
    remotesLoading.value = true;
    try {
      remotes.value = (await commands.repositoryRemotesLoad(repo)).remotes;
    } catch {
      remotes.value = [];
    } finally {
      remotesLoading.value = false;
    }
  }

  async function open(path: string): Promise<RepoInfo | null> {
    busy.value = true;
    lastError.value = null;
    try {
      const info = await commands.repositoryOpen(path);
      upsert(info);
      selectedId.value = info.id;
      await loadRemotes(info.id);
      return info;
    } catch (error) {
      // The launch screen renders `lastError` as an alert, so this failure is
      // already on screen; claiming it keeps the fallback toast from repeating it.
      lastError.value = error as ErrorPayload;
      markHandled(error);
      return null;
    } finally {
      busy.value = false;
    }
  }

  async function init(path: string): Promise<RepoInfo | null> {
    busy.value = true;
    lastError.value = null;
    try {
      const info = await commands.repositoryInit(path, { createGitignore: true });
      upsert(info);
      selectedId.value = info.id;
      await loadRemotes(info.id);
      return info;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      markHandled(error);
      return null;
    } finally {
      busy.value = false;
    }
  }

  function select(id: RepoId): void {
    selectedId.value = id;
    void loadRemotes(id);
  }

  /**
   * Clone `url` and open what comes back.
   *
   * The destination is resolved by Rust (`repository_clone_suggest_destination`)
   * and then confirmed through the folder picker, so the user always sees where
   * the clone is going before a single byte moves. Cloning straight to a
   * guessed path is how you end up with a stray `meridian.git` in Downloads.
   */
  async function clone(
    url: string,
    destination?: string,
  ): Promise<RepoInfo | null> {
    const trimmed = url.trim();
    if (!trimmed) return null;

    let target = destination;
    if (!target) {
      const suggested = await commands.repositoryCloneDestination(trimmed);
      const picked = await commands.pickDirectory();
      // A cancelled picker is not a failed clone — it is the user changing
      // their mind, and it must not leave an error on screen.
      if (!picked) return null;
      target = `${picked.replace(/\/+$/, "")}/${suggested}`;
    }

    busy.value = true;
    lastError.value = null;
    try {
      const info = await commands.repositoryClone(trimmed, { destination: target });
      upsert(info);
      selectedId.value = info.id;
      await loadRemotes(info.id);
      return info;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      markHandled(error);
      return null;
    } finally {
      busy.value = false;
    }
  }

  /**
   * Walk a root for repositories, for the "found nearby" list.
   *
   * Results are roots, not open repositories: scanning a home directory should
   * offer rows to open, not silently add thirty entries to the sidebar.
   */
  const nearby = ref<RepoRoot[]>([]);
  const nearbyState = ref<"idle" | "scanning" | "done" | "empty">("idle");

  async function discoverNearby(root: string): Promise<void> {
    nearbyState.value = "scanning";
    try {
      const found = await commands.repositoryDiscover(root);
      nearby.value = found;
      nearbyState.value = found.length === 0 ? "empty" : "done";
    } catch (error) {
      nearby.value = [];
      nearbyState.value = "empty";
      lastError.value = error as ErrorPayload;
      markHandled(error);
    }
  }

  /** Drop a repository from the recent list (it is closed, not deleted on disk). */
  function remove(id: RepoId): void {
    repositories.value = repositories.value.filter((repo) => repo.id !== id);
    if (selectedId.value === id) {
      selectedId.value = repositories.value[0]?.id ?? null;
    }
  }

  /** Reveal a repository's working directory in the OS file manager. */
  async function reveal(id: RepoId): Promise<ErrorPayload | null> {
    lastError.value = null;
    try {
      // `.` resolves to the working directory, which Finder opens directly.
      await commands.wcEntryOpen(id, ".", "finder");
      return null;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      markHandled(error);
      return lastError.value;
    }
  }

  return {
    repositories,
    selectedId,
    selected,
    busy,
    lastError,
    remotes,
    remotesLoading,
    defaultRemote,
    nearby,
    nearbyState,
    open,
    init,
    clone,
    discoverNearby,
    select,
    refreshInfo,
    remove,
    reveal,
    upsert,
    loadRemotes,
  };
});
