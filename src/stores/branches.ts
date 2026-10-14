import { computed, ref } from "vue";
import { defineStore } from "pinia";

import type { BranchInfo, ErrorPayload, RepoId } from "../bridges/bindings";
import * as commands from "../bridges/commands";
import { reportFailure } from "../diagnostics/failures";
import { dataState, errorState, type LoadState } from "./loadState";
import { useReposStore } from "./repos";

export const useBranchesStore = defineStore("branches", () => {
  const repos = useReposStore();
  const state = ref<LoadState<BranchInfo[]>>({ status: "empty" });
  /** Local branches only: the create/checkout/rename verbs act on these. */
  const branches = ref<BranchInfo[]>([]);
  const remoteBranches = ref<BranchInfo[]>([]);
  const lastError = ref<ErrorPayload | null>(null);

  /**
   * Every branch the repository has.
   *
   * A branch list that silently held only local branches made the Remote and
   * All filters render an empty list — indistinguishable from "this repository
   * has no remotes", which is a different fact and a common one.
   */
  const allBranches = computed(() => [...branches.value, ...remoteBranches.value]);

  async function refresh(repo: RepoId, search?: string): Promise<void> {
    state.value = { status: "loading" };
    try {
      const [locals, remotes] = await Promise.all([
        commands.branchList(repo, "local", search),
        commands.branchList(repo, "remote", search),
      ]);
      branches.value = locals;
      remoteBranches.value = remotes;
      state.value = dataState([...locals, ...remotes]);
    } catch (error) {
      // The list is emptied on purpose: leaving the previous repository's
      // branches under the new one's name would be worse. The error arm says
      // why, and if the pane is not mounted the net still toasts.
      branches.value = [];
      remoteBranches.value = [];
      state.value = errorState(error, "branch_list");
    }
  }

  /**
   * Create a branch at HEAD.
   *
   * `checkout` is the dialogue's "check out after creation" checkbox, and is
   * passed through rather than hardcoded: a tick that changed nothing is worse
   * than not offering it.
   */
  async function create(repo: RepoId, name: string, checkout = true): Promise<BranchInfo | null> {
    lastError.value = null;
    try {
      const info = await commands.branchCreate(repo, name, { kind: "head" }, checkout, "dialogue");
      if (checkout) await repos.refreshInfo(repo);
      await refresh(repo);
      return info;
    } catch (error) {
      lastError.value = reportFailure(error, "branch_create");
      return null;
    }
  }

  async function checkout(repo: RepoId, name: string): Promise<boolean> {
    lastError.value = null;
    try {
      await commands.branchCheckout(repo, name, {});
      // HEAD moved: the repository's own view of it has to move with it, or the
      // toolbar pushes whatever branch was checked out when it was opened.
      await repos.refreshInfo(repo);
      await refresh(repo);
      return true;
    } catch (error) {
      lastError.value = reportFailure(error, "branch_checkout");
      return false;
    }
  }

  async function checkoutRemote(repo: RepoId, remoteBranch: string): Promise<boolean> {
    lastError.value = null;
    const shorthand = remoteBranch.replace(/^refs\/remotes\//, "");
    const separator = shorthand.indexOf("/");
    const suggestedName = separator < 0 ? shorthand : shorthand.slice(separator + 1);
    const nameExists = branches.value.some((branch) => branch.name === suggestedName);
    try {
      await commands.branchCheckoutRemote(repo, shorthand, nameExists ? undefined : suggestedName, true);
      await repos.refreshInfo(repo);
      await refresh(repo);
      return true;
    } catch (error) {
      lastError.value = reportFailure(error, "branch_checkout_remote");
      return false;
    }
  }

  async function rename(repo: RepoId, old: string, next: string, force = false): Promise<BranchInfo | null> {
    lastError.value = null;
    try {
      const info = await commands.branchRename(repo, old, next, force);
      if (old === repos.selected?.headBranch) await repos.refreshInfo(repo);
      await refresh(repo);
      return info;
    } catch (error) {
      lastError.value = reportFailure(error, "branch_rename");
      return null;
    }
  }

  async function remove(repo: RepoId, names: string[], force = false): Promise<boolean> {
    lastError.value = null;
    try {
      await commands.branchDelete(repo, names, force);
      if (names.includes(repos.selected?.headBranch ?? "")) await repos.refreshInfo(repo);
      await refresh(repo);
      return true;
    } catch (error) {
      lastError.value = reportFailure(error, "branch_delete");
      return false;
    }
  }

  return {
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
});
