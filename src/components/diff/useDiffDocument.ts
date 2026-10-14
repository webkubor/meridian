/**
 * Loading and option state for a working-copy diff, for hosts that want
 * `<DiffView>` to fetch its own document. Hosts that already hold a document
 * (the history pane) pass it as a prop instead and never construct this.
 *
 * `contextLines`, `ignoreWhitespace` and `showFull` are engine options — the
 * command recomputes the diff — so changing one re-requests the document. The
 * renderer-side toggles (layout, word diff, wrapping, show-all) live in the
 * view model and never re-request.
 */

import { ref, watch, type Ref } from "vue";

import type { ErrorPayload, RepoId, StatusDelta } from "../../bridges/bindings";
import { tauriDiffPort, type DiffPort } from "./diffCommands";
import { CONTEXT_PRESETS, nextContextLines } from "./diffModel";
import type {
  ChunkDirection,
  DiffDocument,
  DiffOptions,
  DiffPerspective,
  HunkRef,
} from "./types";

export type DiffLoadStatus = "idle" | "loading" | "ready" | "empty" | "error";

export interface DiffRequest {
  repo: RepoId | null | undefined;
  path: string | null | undefined;
  perspective: DiffPerspective;
}

export interface DiffDocumentHandle {
  status: Ref<DiffLoadStatus>;
  document: Ref<DiffDocument | null>;
  error: Ref<ErrorPayload | null>;
  options: Ref<DiffOptions>;
  reload: () => Promise<void>;
  setOptions: (patch: Partial<DiffOptions>) => Promise<void>;
  expandContext: () => Promise<void>;
  requestFull: () => Promise<void>;
  applyChunk: (
    path: string,
    hunk: HunkRef,
    direction: ChunkDirection,
  ) => Promise<StatusDelta | null>;
}

const DEFAULT_OPTIONS: DiffOptions = { contextLines: CONTEXT_PRESETS[1], showFull: false };

export function useDiffDocument(
  request: () => DiffRequest,
  port: DiffPort = tauriDiffPort,
): DiffDocumentHandle {
  const status = ref<DiffLoadStatus>("idle");
  const document = ref<DiffDocument | null>(null);
  const error = ref<ErrorPayload | null>(null);
  const options = ref<DiffOptions>({ ...DEFAULT_OPTIONS });
  let generation = 0;

  async function load(): Promise<void> {
    const { repo, path, perspective } = request();
    const current = ++generation;
    if (!repo || !path) {
      status.value = "idle";
      document.value = null;
      error.value = null;
      return;
    }
    status.value = "loading";
    error.value = null;
    try {
      const result = await port.loadDiff(repo, path, perspective, { ...options.value });
      if (current !== generation) return;
      document.value = result;
      status.value = result.kind === "empty" ? "empty" : "ready";
    } catch (cause) {
      if (current !== generation) return;
      error.value = asErrorPayload(cause);
      status.value = "error";
      document.value = null;
    }
  }

  watch(
    () => {
      const { repo, path, perspective } = request();
      return `${repo ?? ""}\u0000${path ?? ""}\u0000${perspective}`;
    },
    () => void load(),
    { immediate: true },
  );

  async function setOptions(patch: Partial<DiffOptions>): Promise<void> {
    options.value = { ...options.value, ...patch };
    await load();
  }

  async function expandContext(): Promise<void> {
    const current = options.value.contextLines ?? 0;
    const next = nextContextLines(current);
    if (next === current) return;
    await setOptions({ contextLines: next, showFull: false });
  }

  async function requestFull(): Promise<void> {
    await setOptions({ showFull: true });
  }

  async function applyChunk(
    path: string,
    hunk: HunkRef,
    direction: ChunkDirection,
  ): Promise<StatusDelta | null> {
    const repo = request().repo;
    if (!repo) return null;
    try {
      const delta = await port.applyChunk(repo, path, hunk, direction);
      await load();
      return delta;
    } catch (cause) {
      error.value = asErrorPayload(cause);
      status.value = "error";
      return null;
    }
  }

  return { status, document, error, options, reload: load, setOptions, expandContext, requestFull, applyChunk };
}

/** The Rust error model is the app's contract (docs/02 §14); anything thrown
 *  that does not look like a payload is wrapped for display. */
export function asErrorPayload(cause: unknown): ErrorPayload {
  if (
    cause != null &&
    typeof cause === "object" &&
    "code" in cause &&
    "message" in cause &&
    "domain" in cause
  ) {
    return cause as ErrorPayload;
  }
  return {
    code: "unclassified",
    message: cause instanceof Error ? cause.message : String(cause),
    retryable: false,
    domain: "internal",
  };
}
