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
  const remoteBranches = computed(() => store.remoteBranches);
  const allBranches = computed(() => store.allBranches);
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

  async function create(name: string, checkout = true): Promise<BranchInfo | null> {
    const id = repos.selectedId;
    if (!id) return null;
    return store.create(id, name, checkout);
  }

  async function checkout(name: string): Promise<boolean> {
    const id = repos.selectedId;
    if (!id) return false;
    return store.checkout(id, name);
  }

  /**
   * Check out a remote-tracking branch, creating a local branch at its tip.
   *
   * Distinct from `checkout` because the verb has to create something: there is
   * no local `origin/main` to switch to.
   */
  async function checkoutRemote(fullName: string): Promise<boolean> {
    const id = repos.selectedId;
    if (!id) return false;
    return store.checkoutRemote(id, fullName);
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

  return {
    search,
    state,
    branches,
    remoteBranches,
    allBranches,
    lastError,
    refresh,
    create,
    checkout,
    checkoutRemote,
    rename,
    remove,
  };
}
