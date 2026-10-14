import { onMounted, onUnmounted } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { onAppEvent } from "../bridges/events";
import { useBranchesStore } from "../stores/branches";
import { useHistoryStore } from "../stores/history";
import { useReposStore } from "../stores/repos";
import { useWorkingCopyStore } from "../stores/workingCopy";

/**
 * Wire the Rust-side event bus to the stores, once, from the shell.
 * Stores never poll: a `RefsChanged` from the backend is what reloads the
 * branches list, exactly as docs/02 §13 describes.
 */
export function useAppEvents(): void {
  let unlisten: UnlistenFn | null = null;

  const repos = useReposStore();
  const branches = useBranchesStore();
  const workingCopy = useWorkingCopyStore();
  const history = useHistoryStore();

  onMounted(async () => {
    unlisten = await onAppEvent((event) => {
      switch (event.kind) {
        case "reposChanged":
          // The store that ran the command already updated itself; nothing to
          // do until multi-window support lands.
          break;
        case "refsChanged": {
          if (event.payload.repo !== repos.selectedId) return;
          void branches.refresh(event.payload.repo);
          void history.refresh(event.payload.repo);
          break;
        }
        case "workingCopyChanged": {
          if (event.payload.repo !== repos.selectedId) return;
          void workingCopy.refresh(event.payload.repo);
          break;
        }
      }
    });
  });

  onUnmounted(() => {
    unlisten?.();
  });
}
