import { computed } from "vue";

import type { DiffPerspective } from "../bridges/bindings";
import type { DiffOptions, HunkAction } from "../components/contracts";
import { useDiffStore } from "../stores/diff";
import { useReposStore } from "../stores/repos";

/** The working-copy diff data layer, bound to the selected repository. */
export function useDiff() {
  const repos = useReposStore();
  const store = useDiffStore();

  const document = computed(() => store.document);
  const options = computed(() => store.options);

  async function load(path: string, perspective: DiffPerspective): Promise<void> {
    const id = repos.selectedId;
    if (id) await store.load(id, path, perspective);
  }

  function clear(): void {
    store.clear();
  }

  async function setOptions(patch: Partial<DiffOptions>): Promise<void> {
    await store.setOptions(patch);
  }

  async function runHunk(
    path: string,
    hunk: { oldStart: number; oldLines: number; newStart: number; newLines: number },
    action: HunkAction,
  ) {
    const id = repos.selectedId;
    if (!id) return null;
    return store.runHunkAction(id, path, hunk, action);
  }

  return { document, options, load, clear, setOptions, runHunk, reload: store.reload };
}
