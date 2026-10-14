import { computed, watch } from "vue";

import { useReposStore } from "../stores/repos";
import { useWorkingCopyStore, type CommitRequest } from "../stores/workingCopy";

/** The working-copy data layer, bound to the selected repository. */
export function useWorkingCopy() {
  const repos = useReposStore();
  const store = useWorkingCopyStore();

  const state = computed(() => store.state);
  const entries = computed(() => store.entries);
  const summary = computed(() => store.summary);
  const lastError = computed(() => store.lastError);

  watch(
    () => repos.selectedId,
    (id) => {
      if (id) void store.refresh(id);
    },
    { immediate: true },
  );

  function activeRepo(): string | null {
    return repos.selectedId;
  }

  async function refresh(): Promise<void> {
    const id = activeRepo();
    if (id) await store.refresh(id);
  }

  async function stage(path: string): Promise<void> {
    const id = activeRepo();
    if (id) await store.stage(id, [path]);
  }

  async function unstage(path: string): Promise<void> {
    const id = activeRepo();
    if (id) await store.unstage(id, [path]);
  }

  async function discard(path: string): Promise<boolean> {
    const id = activeRepo();
    if (!id) return false;
    return store.discard(id, [path]);
  }

  async function commit(request: CommitRequest): Promise<boolean> {
    const id = activeRepo();
    if (!id) return false;
    return (await store.commit(id, request)) !== null;
  }

  /**
   * Clear or set git's assume-unchanged bit.
   *
   * A file carrying that bit is invisible to `git status`, so without a way to
   * clear it a user can lose track of a file entirely — the flag was previously
   * shown as a permanently greyed button.
   */
  async function setAssumeUnchanged(paths: string[], value: boolean): Promise<boolean> {
    const id = repos.selectedId;
    if (!id) return false;
    return store.setAssumeUnchanged(id, paths, value);
  }

  async function openInEditor(path: string): Promise<void> {
    const id = activeRepo();
    if (!id) return;
    await store.openEntry(id, path, "editor");
  }

  return {
    state,
    entries,
    summary,
    lastError,
    refresh,
    stage,
    unstage,
    discard,
    commit,
    setAssumeUnchanged,
    openInEditor,
  };
}
