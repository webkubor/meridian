import { ref } from "vue";
import { defineStore } from "pinia";

import type { CommitResult, EntryStatus, ErrorPayload, OpenTarget, RepoId, StatusDelta, StatusSummary } from "../bridges/bindings";
import * as commands from "../bridges/commands";
import { dataState, errorState, type LoadState } from "./loadState";

export interface CommitRequest {
  message: string;
  amend: boolean;
  signOff: boolean;
}

export const useWorkingCopyStore = defineStore("workingCopy", () => {
  const state = ref<LoadState<EntryStatus[]>>({ status: "empty" });
  const entries = ref<EntryStatus[]>([]);
  const summary = ref<StatusSummary | null>(null);
  const lastError = ref<ErrorPayload | null>(null);

  function apply(delta: StatusDelta): void {
    entries.value = delta.entries;
    summary.value = delta.summary;
    state.value = dataState(delta.entries);
  }

  async function refresh(repo: RepoId): Promise<void> {
    state.value = { status: "loading" };
    try {
      apply(await commands.wcStatusLoad(repo));
    } catch (error) {
      state.value = errorState(error);
    }
  }

  async function stage(repo: RepoId, paths: string[]): Promise<void> {
    try {
      apply(await commands.wcStagePaths(repo, paths));
    } catch (error) {
      state.value = errorState(error);
    }
  }

  async function unstage(repo: RepoId, paths: string[]): Promise<void> {
    try {
      apply(await commands.wcUnstagePaths(repo, paths));
    } catch (error) {
      state.value = errorState(error);
    }
  }

  /** Discard the working-tree changes in `paths` (docs/03 §3, destructive). */
  async function discard(repo: RepoId, paths: string[]): Promise<boolean> {
    lastError.value = null;
    try {
      apply(await commands.wcPathsDiscard(repo, paths));
      return true;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      return false;
    }
  }

  /**
   * Create (or amend) a commit from the composer. The command commits the
   * index, so the request carries the message and options, not a path list;
   * the staged set is whatever `wc_status_load` last reported.
   */
  async function commit(repo: RepoId, request: CommitRequest): Promise<CommitResult | null> {
    lastError.value = null;
    try {
      const opts = { signOff: request.signOff };
      // An amend with no message reuses the commit being replaced, so send
      // `null` (the engine reads the existing message) rather than an empty one.
      const message = request.amend && request.message.trim() === "" ? null : request.message;
      const result = request.amend
        ? await commands.commitAmend(repo, message, opts)
        : await commands.commitCreate(repo, request.message, opts);
      await refresh(repo);
      return result;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      return null;
    }
  }

  /** Launch a path in the editor / terminal / Finder (docs/03 §3). */
  async function openEntry(repo: RepoId, path: string, target: OpenTarget): Promise<void> {
    lastError.value = null;
    try {
      await commands.wcEntryOpen(repo, path, target);
    } catch (error) {
      lastError.value = error as ErrorPayload;
    }
  }

  /** Set or clear git's assume-unchanged bit on paths. */
  async function setAssumeUnchanged(
    repo: RepoId,
    paths: string[],
    value: boolean,
  ): Promise<boolean> {
    lastError.value = null;
    try {
      await commands.wcEntryAssumeUnchangedSet(repo, paths, value);
      await refresh(repo);
      return true;
    } catch (error) {
      lastError.value = error as ErrorPayload;
      return false;
    }
  }

  return {
    state,
    entries,
    summary,
    lastError,
    refresh,
    stage,
    unstage,
    discard,
    commit,
    openEntry,
    setAssumeUnchanged,
  };
});
