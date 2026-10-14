import { ref } from "vue";
import { defineStore } from "pinia";

import type { BranchInfo, ErrorPayload, RepoId } from "../bridges/bindings";
import * as commands from "../bridges/commands";
import { dataState, errorState, type LoadState } from "./loadState";

export const useBranchesStore = defineStore("branches", () => {
  const state = ref<LoadState<BranchInfo[]>>({ status: "empty" });
  const branches = ref<BranchInfo[]>([]);
  const lastError = ref<ErrorPayload | null>(null);

  async function refresh(repo: RepoId, search?: string): Promise<void> {
    state.value = { status: "loading" };
    try {
      branches.value = await commands.branchList(repo, "local", search);
      state.value = dataState(branches.value);
    } catch (error) {
      state.value = errorState(error);
    }
  }

  async function create(repo: RepoId, name: string): Promise<BranchInfo | null> {
    lastError.value = null;
    try {
      const info = await commands.branchCreate(repo, name, { kind: "head" }, true, "dialogue");
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

  return { state, branches, lastError, refresh, create, checkout, rename, remove };
});
