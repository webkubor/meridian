import { ref } from "vue";

import type { ErrorPayload, RepoId } from "../bridges/bindings";
import * as commands from "../bridges/commands";
import { reportFailure } from "../diagnostics/failures";
import type { IntegrationResult } from "../bridges/bindings";
import { useBranchesStore } from "../stores/branches";
import { useHistoryStore } from "../stores/history";
import { useReposStore } from "../stores/repos";
import { useWorkingCopyStore } from "../stores/workingCopy";

/**
 * Repo-level git verbs (docs/03 §8 remote_*, §12 integrate_*): the actions the
 * toolbar, the integration banner and the branch panes trigger. Each returns
 * the backend `ErrorPayload` on failure (never a generic message) so the caller
 * can surface exactly what the engine refused, or a `conflicted` flag when the
 * engine stopped with conflicts to resolve.
 *
 * These verbs change refs / the working tree, so every success refreshes the
 * branches, working-copy and history stores — the UI never has to guess.
 */
export function useGit() {
  const repos = useReposStore();
  const branches = useBranchesStore();
  const workingCopy = useWorkingCopyStore();
  const history = useHistoryStore();

  const lastError = ref<ErrorPayload | null>(null);
  const busy = ref(false);

  function repoId(): RepoId | null {
    return repos.selectedId;
  }

  async function refreshAll(repo: RepoId): Promise<void> {
    // The repository record carries HEAD. Refresh it alongside the other
    // stores: a pull that moves HEAD leaves `repos.selected.headBranch` naming
    // the old branch, and the next Push publishes that instead of the branch the
    // user is actually on.
    await Promise.all([
      repos.refreshInfo(repo),
      branches.refresh(repo),
      workingCopy.refresh(repo),
      history.refresh(repo),
    ]);
  }

  async function run(action: (repo: RepoId) => Promise<unknown>): Promise<VerbResult> {
    const id = repoId();
    if (!id) return { error: null, conflicted: false };
    busy.value = true;
    lastError.value = null;
    try {
      const result = await action(id);
      await refreshAll(id);
      return { error: null, conflicted: isConflicted(result) };
    } catch (error) {
      lastError.value = reportFailure(error, "git verb");
      return { error: lastError.value, conflicted: false };
    } finally {
      busy.value = false;
    }
  }

  return {
    lastError,
    busy,
    fetch: () =>
      run((id) => {
        const remote = repos.defaultRemote;
        if (!remote) throw missingRemote();
        return commands.remoteFetch(id, remote);
      }),
    pull: () =>
      run((id) => {
        const remote = repos.defaultRemote;
        if (!remote) throw missingRemote();
        return commands.remotePull(id, remote);
      }),
    /**
     * Publish a branch. Defaults to the checked-out one; a branch pane must pass
     * the branch it is showing, because pushing from a *selected* branch while
     * the user reads a *different* branch's pane publishes the wrong ref — the
     * worst kind of quiet wrongness for a tool whose whole promise is not
     * surprising anyone.
     */
    push: (branchName?: string) =>
      run((id) => {
        const remote = repos.defaultRemote;
        if (!remote) throw missingRemote();
        const branch = branchName ?? repos.selected?.headBranch;
        if (!branch) {
          throw { code: "invalid_request", message: "HEAD is detached — check out a branch to push.", domain: "git", retryable: false } as ErrorPayload;
        }
        return commands.remotePublishBranch(id, remote, branch);
      }),
    stash: () => run((id) => commands.stashSave(id, {})),
    merge: (name: string) =>
      run((id) => commands.mergeStart(id, { kind: "branch", value: name }, { autoCommit: true })),
    rebase: (name: string) =>
      run((id) => commands.rebaseStart(id, { kind: "branch", value: name }, null, {})),
    continueOperation: () => run((id) => commands.integrationContinue(id)),
    abortOperation: () => run((id) => commands.mergeAbort(id)),
    resolveConflict: (paths: string[]) => run((id) => commands.mergeToolInvoke(id, paths)),
  };
}

/**
 * What a repo-level verb came back with.
 *
 * `conflicted` matters because `merge_start`/`rebase_start`/`pull` return
 * `Ok(IntegrationResult{ outcome: Conflicted })` when conflicts stopped the
 * operation: an `Ok` that means "not finished" must never be reported as a
 * success.
 */
export interface VerbResult {
  error: ErrorPayload | null;
  conflicted: boolean;
}

function isConflicted(result: unknown): boolean {
  const outcome = (result as IntegrationResult | undefined)?.outcome;
  return typeof outcome === "object" && outcome !== null && "kind" in outcome && outcome.kind === "conflicted";
}

function missingRemote(): ErrorPayload {
  return {
    code: "invalid_request",
    message: "This repository has no remote configured.",
    domain: "git",
    retryable: false,
  };
}
