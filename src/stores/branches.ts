import { computed, ref } from "vue";
import { defineStore } from "pinia";

import type { BranchInfo, ErrorPayload, RepoId } from "../bridges/bindings";
import * as commands from "../bridges/commands";
import { dataState, errorState, type LoadState } from "./loadState";

export const useBranchesStore = defineStore("branches", () => {
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
      branches.value = [];
      remoteBranches.value = [];
      state.value = errorState(error);
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
      await refresh(repo);
      return info;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      return null;
    }
  }

  async function checkout(repo: RepoId, name: string): Promise<boolean> {
    lastError.value = null;
    try {
      await commands.branchCheckout(repo, name, {});
      await refresh(repo);
      return true;
    } catch (error) {
      lastError.value = error as ErrorPayload;
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
      await refresh(repo);
      return true;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      return false;
    }
  }

  async function rename(repo: RepoId, old: string, next: string, force = false): Promise<BranchInfo | null> {
    lastError.value = null;
    try {
      const info = await commands.branchRename(repo, old, next, force);
      await refresh(repo);
      return info;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      return null;
    }
  }

  async function remove(repo: RepoId, names: string[], force = false): Promise<boolean> {
    lastError.value = null;
    try {
      await commands.branchDelete(repo, names, force);
      await refresh(repo);
      return true;
    } catch (error) {
      lastError.value = error as ErrorPayload;
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
