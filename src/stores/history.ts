import { computed, ref } from "vue";
import { defineStore } from "pinia";

import type {
  CommitInfo,
  CommitPage,
  HistoryCommitsOptions,
  RepoId,
  Revision,
} from "../bridges/bindings";
import * as commands from "../bridges/commands";
import type { Changeset, CommitDetail, GraphLayout } from "../components/contracts";
import { dataState, errorState, type LoadState } from "./loadState";
import { toChangeset, toCommitDetail, toGraphLayout } from "./viewModels";

const PAGE_SIZE = 250;

/**
 * History data + selection (docs/04 §S8).
 *
 * The store owns three things the panes only render:
 *   - the loaded pages of commits and the "load more" cursor;
 *   - the lane layout for exactly those commits (`history_graph_compute`);
 *   - the selected commit, its detail and changeset, and the selected file
 *     inside that changeset.
 *
 * Selection lives here — not in `App.vue` and never component-local — so the
 * keyboard cursor, the detail pane and the graph all read one source of truth.
 */
export const useHistoryStore = defineStore("history", () => {
  const state = ref<LoadState<CommitInfo[]>>({ status: "empty" });
  const commits = ref<CommitInfo[]>([]);
  const truncated = ref(false);
  const from = ref<Revision | null>(null);

  const graph = ref<GraphLayout | null>(null);

  const selectedOid = ref<string | null>(null);
  const selectedPath = ref<string | null>(null);
  const detail = ref<LoadState<CommitDetail>>({ status: "empty" });
  const changeset = ref<LoadState<Changeset>>({ status: "empty" });

  /**
   * The filters the current page was loaded with (search text, sort,
   * first-parent). Held here so "load more" pages the same query the top of the
   * list was built from, and so a filter change re-runs the load.
   */
  const query = ref<HistoryCommitsOptions>({ limit: PAGE_SIZE });

  const selectedCommit = computed<CommitInfo | null>(
    () => commits.value.find((commit) => commit.oid === selectedOid.value) ?? null,
  );

  function apply(page: CommitPage): void {
    commits.value = page.commits;
    truncated.value = page.truncated;
    state.value = dataState(page.commits);
    reconcileSelection();
  }

  /** Drop a selection that is no longer in the loaded set. */
  function reconcileSelection(): void {
    if (selectedOid.value && !commits.value.some((commit) => commit.oid === selectedOid.value)) {
      selectedOid.value = null;
      selectedPath.value = null;
      detail.value = { status: "empty" };
      changeset.value = { status: "empty" };
    }
  }

  async function loadGraph(repo: RepoId): Promise<void> {
    const oids = commits.value.map((commit) => commit.oid);
    if (oids.length === 0) {
      graph.value = null;
      return;
    }
    try {
      graph.value = toGraphLayout(await commands.historyGraphCompute(repo, oids));
    } catch {
      // The list stays usable without lanes; it is not an error worth a toast.
      graph.value = null;
    }
  }

  async function refresh(repo: RepoId, options?: HistoryCommitsOptions): Promise<void> {
    if (options) query.value = { ...options, limit: options.limit ?? PAGE_SIZE };
    state.value = { status: "loading" };
    try {
      apply(await commands.historyCommitsLoad(repo, query.value));
      await loadGraph(repo);
    } catch (error) {
      state.value = errorState(error);
      graph.value = null;
    }
  }

  /** Walk onward from the current tail (the "load more" affordance). */
  async function loadMore(repo: RepoId): Promise<void> {
    const tail = commits.value[commits.value.length - 1];
    if (!tail) return;
    try {
      const page = await commands.historyCommitsLoad(repo, {
        ...query.value,
        from: { kind: "oid", value: tail.oid },
        skip: 0,
        limit: PAGE_SIZE + 1,
      });
      const fresh = page.commits.slice(1); // the tail itself comes back first
      commits.value = [...commits.value, ...fresh];
      truncated.value = page.truncated;
      state.value = dataState(commits.value);
      await loadGraph(repo);
    } catch (error) {
      state.value = errorState(error);
    }
  }

  /** Select a commit and load its detail + changeset. */
  async function select(repo: RepoId, oid: string): Promise<void> {
    selectedOid.value = oid;
    selectedPath.value = null;
    detail.value = { status: "loading" };
    changeset.value = { status: "loading" };
    await Promise.all([loadDetail(repo, oid), loadChangeset(repo, oid)]);
  }

  async function loadDetail(repo: RepoId, oid: string): Promise<void> {
    try {
      detail.value = { status: "data", data: toCommitDetail(await commands.historyCommitLoad(repo, oid)) };
    } catch (error) {
      detail.value = errorState(error);
    }
  }

  async function loadChangeset(repo: RepoId, oid: string): Promise<void> {
    try {
      changeset.value = { status: "data", data: toChangeset(await commands.historyChangesetLoad(repo, oid)) };
    } catch (error) {
      changeset.value = errorState(error);
    }
  }

  function selectPath(path: string | null): void {
    selectedPath.value = path;
  }

  return {
    state,
    commits,
    truncated,
    from,
    graph,
    selectedOid,
    selectedPath,
    selectedCommit,
    detail,
    changeset,
    refresh,
    loadMore,
    loadGraph,
    select,
    selectPath,
    loadDetail,
    loadChangeset,
  };
});
