import { computed, ref, watch } from "vue";
import type { BranchInfo } from "../bridges/bindings";

import { useBranchesStore } from "../stores/branches";
import { useReposStore } from "../stores/repos";

/** The branches data layer: list, create, checkout — bound to the selection. */
export function useBranches() {
  const repos = useReposStore();
  const store = useBranchesStore();
  const search = ref("");

  const state = computed(() => store.state);
  const branches = computed(() => store.branches);
  const lastError = computed(() => store.lastError);

  watch(
    () => repos.selectedId,
    (id) => {
      if (id) void store.refresh(id);
    },
    { immediate: true },
  );

  watch(search, () => {
    const id = repos.selectedId;
    if (id) void store.refresh(id, search.value);
  });

  async function refresh(): Promise<void> {
    const id = repos.selectedId;
    if (id) await store.refresh(id, search.value);
  }

  async function create(name: string): Promise<BranchInfo | null> {
    const id = repos.selectedId;
    if (!id) return null;
    return store.create(id, name);
  }

  async function checkout(name: string): Promise<boolean> {
    const id = repos.selectedId;
    if (!id) return false;
    return store.checkout(id, name);
  }

  async function rename(old: string, next: string): Promise<BranchInfo | null> {
    const id = repos.selectedId;
    if (!id) return null;
    return store.rename(id, old, next);
  }

  async function remove(name: string): Promise<boolean> {
    const id = repos.selectedId;
    if (!id) return false;
    return store.remove(id, [name]);
  }

  return { search, state, branches, lastError, refresh, create, checkout, rename, remove };
}
