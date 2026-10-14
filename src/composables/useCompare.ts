/**
 * The compare screen's state: two ref choices and the comparison between them.
 *
 * This was a screen with three working-looking ref pickers, a swap button and a
 * commit list, none of which had anything behind them — `comparison` was
 * always `null`, `left` and `right` were always `null`, and every handler was
 * `() => {}`. A control that renders and does nothing is worse than one that is
 * absent, so this composable exists to give it something real.
 *
 * Two different questions, answered by two different things:
 *   · How many? `history_merge_base` — git2's `graph_ahead_behind` against the
 *     real merge base. "The newest commit of one side" is not a count and is
 *     not the fork point; reporting either would be a lie.
 *   · Which? Two ordinary history pages over the ranges, paged so a long
 *     divergence costs a screenful of objects rather than all of them.
 */

import { computed, ref, watch } from "vue";

import * as commands from "../bridges/commands";
import type {
  BranchInfo,
  CommitInfo,
  RepoId,
  Revision,
} from "../bridges/bindings";
import type { Comparison, CompareEntry, LoadState, RefChoice } from "../components/contracts";
import { errorState } from "../stores/loadState";

/**
 * A branch as a ref the compare screen can pick.
 *
 * `id` is the full ref rather than the short name on purpose: a repository with
 * both `main` and `origin/main` would otherwise offer two rows whose labels are
 * identical and whose comparisons are not.
 */
export function toRefChoice(branch: BranchInfo): RefChoice {
  return {
    id: branch.fullName,
    label: branch.isRemote ? branch.fullName : branch.name,
    detail: branch.isRemote ? undefined : branch.upstream ?? undefined,
    kind: branch.isRemote ? "remoteBranch" : branch.isHead ? "head" : "branch",
  };
}

function revisionFor(choice: RefChoice): Revision {
  if (choice.kind === "head") return { kind: "head" };
  if (choice.kind === "tag") return { kind: "tag", value: choice.id };
  // A remote-tracking ref is a branch ref; the engine's resolver falls back to
  // git's own when `refs/heads/<fullName>` misses, which is what makes
  // `origin/main` resolvable here.
  return { kind: "branch", value: choice.id };
}

export function useCompare() {
  const left = ref<RefChoice | null>(null);
  const right = ref<RefChoice | null>(null);
  const comparison = ref<Comparison | null>(null);
  const state = ref<LoadState<Comparison>>({ status: "empty" });
  const busy = ref(false);
  let requestToken = 0;

  const ready = computed(() => left.value !== null && right.value !== null);

  /** Every ref the pickers offer: HEAD first, then branches, then remotes. */
  function choicesFor(repo: RepoId | null, branches: BranchInfo[]): RefChoice[] {
    if (repo === null) return [];
    const head = branches.find((branch) => branch.isHead);
    const rest = branches
      .filter((branch) => !branch.isHead)
      .map(toRefChoice)
      .sort((a, b) => a.label.localeCompare(b.label));
    return head ? [toRefChoice(head), ...rest] : rest;
  }

  // No explicit `reload()` in the three setters below: the watcher at the
  // bottom reloads whenever either side's id changes, so calling it here as
  // well fetched every comparison twice.
  function swap(): void {
    const wasLeft = left.value;
    left.value = right.value;
    right.value = wasLeft;
  }

  function choose(side: "left" | "right", choice: RefChoice): void {
    if (side === "left") left.value = choice;
    else right.value = choice;
  }

  /** Both sides at once, so a default pair is one load rather than two. */
  function setBoth(base: RefChoice, target: RefChoice): void {
    left.value = base;
    right.value = target;
  }

  /**
   * Load the comparison, or clear it when a side is still unchosen.
   *
   * Clearing rather than keeping the old comparison is the point: leaving last
   * pick's numbers on screen under this pick's labels is the one thing that
   * makes a compare view actively misleading.
   */
  async function reload(): Promise<void> {
    if (!ready.value) {
      comparison.value = null;
      state.value = { status: "empty" };
      return;
    }
    const repo = currentRepo;
    if (repo === null) return;

    const leftChoice = left.value as RefChoice;
    const rightChoice = right.value as RefChoice;
    const leftRev = revisionFor(leftChoice);
    const rightRev = revisionFor(rightChoice);

    // A slower, older request must not overwrite a newer one: pick `main`,
    // then quickly `dev`, and the `main` answer arriving last would sit under
    // `dev`'s labels — exactly the mislabelled view `reload` exists to prevent.
    requestToken += 1;
    const token = requestToken;

    busy.value = true;
    state.value = { status: "loading" };
    try {
      // Three independent reads, one round trip:
      //   · the two ranges, so the list has something to render — paged, so a
      //     10k-commit divergence does not become 10k objects;
      //   · the summary, for the *exact* counts and the fork point.
      //
      // The page length and the ahead count are deliberately kept apart: a
      // 200-row page is a window, and reporting "ahead by 200" for a branch
      // that is 4,000 commits ahead is the kind of quiet lie this screen
      // exists to stop telling.
      const [aheadPage, behindPage, summary] = await Promise.all([
        commands.historyCommitsLoad(repo, { from: rightRev, to: leftRev, limit: 200 }),
        commands.historyCommitsLoad(repo, { from: leftRev, to: rightRev, limit: 200 }),
        commands.historyCompareSummary(repo, leftRev, rightRev),
      ]);

      const entries: CompareEntry[] = [
        ...aheadPage.commits.map((commit) => ({ side: "ahead" as const, commit })),
        ...behindPage.commits.map((commit) => ({ side: "behind" as const, commit })),
      ];

      if (token !== requestToken) return;
      comparison.value = {
        left: leftChoice.id,
        right: rightChoice.id,
        mergeBase: summary.mergeBase,
        ahead: summary.ahead,
        behind: summary.behind,
        entries,
      };
      state.value = { status: "data", data: comparison.value };
    } catch (error) {
      if (token !== requestToken) return;
      // The compare pane renders this state and claims it; while another tab is
      // in front the net toasts instead, which is the point.
      comparison.value = null;
      state.value = errorState(error, "history_compare_summary");
    } finally {
      if (token === requestToken) busy.value = false;
    }
  }

  /** Set by `App.vue` on each load; the composable stays repository-agnostic. */
  let currentRepo: RepoId | null = null;
  function forRepo(repo: RepoId | null): void {
    if (repo === currentRepo) return;
    currentRepo = repo;
    requestToken += 1;
    busy.value = false;
    left.value = null;
    right.value = null;
    comparison.value = null;
    state.value = { status: "empty" };
  }

  watch(
    [left, right],
    ([nextLeft, nextRight], [previousLeft, previousRight]) => {
      if (nextLeft?.id !== previousLeft?.id || nextRight?.id !== previousRight?.id) {
        void reload();
      }
    },
  );

  return {
    left,
    right,
    comparison,
    state,
    busy,
    ready,
    choicesFor,
    choose,
    setBoth,
    swap,
    reload,
    forRepo,
  };
}

export type { CommitInfo };