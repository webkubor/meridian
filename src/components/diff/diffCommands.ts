/**
 * The diff module's command port.
 *
 * `bridges/commands.ts` is the app's single `invoke` call site (docs/02 §13).
 * It now exports typed wrappers for every command this port needs —
 * `wcDiffLoad`, `wcChunkApply` and `wcImageDataLoad` — and all three are
 * registered in `src-tauri/src/lib.rs::generate_handler![]`. The `invoke`
 * calls below are therefore redundant: a consumer that prefers the bridge can
 * point `tauriDiffPort` at `bridges/commands.ts` and delete this file's direct
 * `invoke` usage. The live diff panes read from `stores/diff.ts` (which calls
 * the bridge wrappers); this port is kept only for the components that still
 * take it, and its argument shapes match the Rust structs exactly.
 *
 * Every method takes the same camelCase argument structs the Rust commands
 * deserialize (`#[serde(rename_all = "camelCase")]`).
 */

import { invoke } from "@tauri-apps/api/core";

import type { RepoId, StatusDelta } from "../../bridges/bindings";
import type {
  ChunkDirection,
  DiffDocument,
  DiffOptions,
  DiffPerspective,
  HunkRef,
  ImagePayload,
  ImageSide,
} from "./types";

export interface DiffPort {
  /** docs/03 §3 `wc_diff_load`. */
  loadDiff(
    repo: RepoId,
    path: string | null,
    perspective: DiffPerspective,
    options: DiffOptions,
  ): Promise<DiffDocument>;
  /** docs/03 §3 `wc_chunk_apply` — `hunk` is the `(old_start, old_lines,
   *  new_start, new_lines)` quad the selectors imply. */
  applyChunk(
    repo: RepoId,
    path: string,
    hunk: HunkRef,
    direction: ChunkDirection,
    options?: { context?: number },
  ): Promise<StatusDelta>;
  /** docs/03 §3 `wc_image_data_load`. */
  loadImage(repo: RepoId, path: string, side: ImageSide): Promise<ImagePayload>;
}

export const tauriDiffPort: DiffPort = {
  loadDiff(repo, path, perspective, options) {
    return invoke<DiffDocument>("wc_diff_load", {
      args: { repo, path, perspective, opts: options },
    });
  },

  applyChunk(repo, path, hunk, direction, options = {}) {
    return invoke<StatusDelta>("wc_chunk_apply", {
      args: {
        repo,
        path,
        hunk,
        direction: direction === "stage" ? "stage" : "unstage",
        opts: options,
      },
    });
  },

  loadImage(repo, path, side) {
    return invoke<ImagePayload>("wc_image_data_load", { args: { repo, path, side } });
  },
};
