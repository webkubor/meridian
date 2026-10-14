import { ref } from "vue";
import { defineStore } from "pinia";

import type { DiffPerspective, HunkRef, RepoId, StatusDelta } from "../bridges/bindings";
import * as commands from "../bridges/commands";
import type { DiffDocument, DiffOptions, HunkAction } from "../components/contracts";
import { errorState, type LoadState } from "./loadState";
import { toDiffDocument, toDiffOptionsWire } from "./viewModels";

const DEFAULT_OPTIONS: DiffOptions = {
  contextLines: 3,
  ignoreWhitespace: false,
  fullFile: false,
  syntax: true,
};

interface DiffRequest {
  repo: RepoId;
  path: string;
  perspective: DiffPerspective;
}

/**
 * The working-copy diff document (docs/03 §3 `wc_diff_load`).
 *
 * The hunk verbs reuse the coordinates the document carries — the engine
 * regenerates the diff at the same context and matches the hunk by its
 * `(old_start, old_lines, new_start, new_lines)` quad, so the `index` field the
 * wire carries is informational only.
 */
export const useDiffStore = defineStore("diff", () => {
  const document = ref<LoadState<DiffDocument>>({ status: "empty" });
  const options = ref<DiffOptions>({ ...DEFAULT_OPTIONS });
  const request = ref<DiffRequest | null>(null);

  async function load(repo: RepoId, path: string, perspective: DiffPerspective): Promise<void> {
    request.value = { repo, path, perspective };
    document.value = { status: "loading" };
    try {
      const wire = await commands.wcDiffLoad(repo, path, perspective, toDiffOptionsWire(options.value));
      document.value = { status: "data", data: toDiffDocument(wire, path) };
    } catch (error) {
      document.value = errorState(error, "wc_diff_load");
    }
  }

  function clear(): void {
    request.value = null;
    document.value = { status: "empty" };
  }

  async function setOptions(patch: Partial<DiffOptions>): Promise<void> {
    options.value = { ...options.value, ...patch };
    const current = request.value;
    if (current) await load(current.repo, current.path, current.perspective);
  }

  async function reload(): Promise<void> {
    const current = request.value;
    if (current) await load(current.repo, current.path, current.perspective);
  }

  /** Turn a rendered hunk back into the engine's hunk identity. */
  function hunkRef(hunk: { oldStart: number; oldLines: number; newStart: number; newLines: number }, path: string): HunkRef {
    return {
      path,
      perspective: request.value?.perspective ?? "workingTreeToIndex",
      index: 0,
      oldStart: hunk.oldStart,
      oldLines: hunk.oldLines,
      newStart: hunk.newStart,
      newLines: hunk.newLines,
      header: "",
    };
  }

  /** Run a hunk verb against the current document; returns the fresh status. */
  async function runHunkAction(
    repo: RepoId,
    path: string,
    hunk: { oldStart: number; oldLines: number; newStart: number; newLines: number },
    action: HunkAction,
  ): Promise<StatusDelta | null> {
    const ref = hunkRef(hunk, path);
    const contextLines = options.value.contextLines;
    try {
      const delta =
        action === "discard"
          ? await commands.wcChunkDiscard(repo, ref, contextLines)
          : await commands.wcChunkApply(repo, ref, action === "stage" ? "stage" : "unstage", contextLines);
      await reload();
      return delta;
    } catch (error) {
      // The diff pane renders this state while it is open; when it is not, the
      // stale-hunk rejection still reaches the user through the net.
      document.value = errorState(error, action === "discard" ? "wc_chunk_discard" : "wc_chunk_apply");
      return null;
    }
  }

  return { document, options, request, load, clear, setOptions, reload, runHunkAction };
});
