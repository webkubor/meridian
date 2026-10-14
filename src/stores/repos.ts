import { computed, ref } from "vue";
import { defineStore } from "pinia";

import type { ErrorPayload, RepoId, RepoInfo, RepositoryRemote } from "../bridges/bindings";
import * as commands from "../bridges/commands";

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
      lastError.value = error as ErrorPayload;
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
      return null;
    } finally {
      busy.value = false;
    }
  }

  function select(id: RepoId): void {
    selectedId.value = id;
    void loadRemotes(id);
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
    open,
    init,
    select,
    remove,
    reveal,
    upsert,
    loadRemotes,
  };
});
