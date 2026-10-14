import { computed, watch } from "vue";

import type { HistoryCommitsOptions } from "../bridges/bindings";
import { useHistoryStore } from "../stores/history";
import { useReposStore } from "../stores/repos";

/** The history data layer: pages, graph, selection — bound to the selection. */
export function useHistory() {
  const repos = useReposStore();
  const store = useHistoryStore();

  const state = computed(() => store.state);
  const commits = computed(() => store.commits);
  const truncated = computed(() => store.truncated);
  const graph = computed(() => store.graph);
  const selectedOid = computed(() => store.selectedOid);
  const selectedPath = computed(() => store.selectedPath);
  const selectedCommit = computed(() => store.selectedCommit);
  const detail = computed(() => store.detail);
  const changeset = computed(() => store.changeset);

  watch(
    () => repos.selectedId,
    (id) => {
      if (id) void store.refresh(id);
    },
    { immediate: true },
  );

  async function refresh(): Promise<void> {
    const id = repos.selectedId;
    if (id) await store.refresh(id);
  }

  /** Re-run the load with new filters (search text, sort, first-parent). */
  async function setQuery(options: HistoryCommitsOptions): Promise<void> {
    const id = repos.selectedId;
    if (id) await store.refresh(id, options);
  }

  async function loadMore(): Promise<void> {
    const id = repos.selectedId;
    if (id) await store.loadMore(id);
  }

  /** Move the selection (arrow keys, a click, a ref click). */
  async function select(oid: string): Promise<void> {
    const id = repos.selectedId;
    if (id) await store.select(id, oid);
  }

  function selectPath(path: string | null): void {
    store.selectPath(path);
  }

  return {
    state,
    commits,
    truncated,
    graph,
    selectedOid,
    selectedPath,
    selectedCommit,
    detail,
    changeset,
    refresh,
    setQuery,
    loadMore,
    select,
    selectPath,
  };
}
