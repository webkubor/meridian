import { computed, ref } from "vue";

import type { ErrorPayload } from "../bridges/bindings";
import { useReposStore } from "../stores/repos";

/**
 * The repository data layer: call sites use these actions instead of talking
 * to `bridges/commands` or the store internals directly.
 */
export function useRepositories() {
  const store = useReposStore();
  const pathInput = ref("");

  const repositories = computed(() => store.repositories);
  const selectedId = computed(() => store.selectedId);
  const selected = computed(() => store.selected);
  const busy = computed(() => store.busy);
  const lastError = computed(() => store.lastError);
  const remotes = computed(() => store.remotes);
  const defaultRemote = computed(() => store.defaultRemote);

  async function open(path?: string): Promise<void> {
    const target = (path ?? pathInput.value).trim();
    if (!target) return;
    const info = await store.open(target);
    if (info) pathInput.value = "";
  }

  async function init(path?: string): Promise<void> {
    const target = (path ?? pathInput.value).trim();
    if (!target) return;
    const info = await store.init(target);
    if (info) pathInput.value = "";
  }

  function select(id: string): void {
    store.select(id);
  }

  /** Remove a repository from the recent list (it is closed, not deleted on disk). */
  function remove(id: string): void {
    store.remove(id);
  }

  /** Reveal a repository's working directory in the OS file manager. */
  async function reveal(id: string): Promise<ErrorPayload | null> {
    return store.reveal(id);
  }

  return {
    pathInput,
    repositories,
    selectedId,
    selected,
    busy,
    lastError,
    remotes,
    defaultRemote,
    open,
    init,
    select,
    remove,
    reveal,
  };
}
