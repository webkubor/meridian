import { computed, ref } from "vue";

import type { ErrorPayload, RepoInfo } from "../bridges/bindings";
import * as commands from "../bridges/commands";
import { useReposStore } from "../stores/repos";

/**
 * The repository data layer: call sites use these actions instead of talking
 * to `bridges/commands` or the store internals directly.
 */
export function useRepositories() {
  const store = useReposStore();
  const pathInput = ref("");
  /** The clone URL field. Local, because a half-typed URL is not app state. */
  const cloneUrl = ref("");

  const repositories = computed(() => store.repositories);
  const selectedId = computed(() => store.selectedId);
  const selected = computed(() => store.selected);
  const busy = computed(() => store.busy);
  const lastError = computed(() => store.lastError);
  const remotes = computed(() => store.remotes);
  const defaultRemote = computed(() => store.defaultRemote);

  /**
   * Opens a repository and returns it, so the caller can navigate straight to
   * it. Returning `void` here is why a caller that opened something by path —
   * a "found nearby" row, say — had no id to navigate with afterwards.
   */
  async function open(path?: string): Promise<RepoInfo | null> {
    const target = (path ?? pathInput.value).trim();
    if (!target) return null;
    const info = await store.open(target);
    if (info) pathInput.value = "";
    return info;
  }

  /** Returns the new repository, so the caller can navigate to it. */
  async function init(path?: string): Promise<RepoInfo | null> {
    const target = (path ?? pathInput.value).trim();
    if (!target) return null;
    const info = await store.init(target);
    if (info) pathInput.value = "";
    return info;
  }

  /**
   * Clone `url`, choosing (and confirming) a destination first.
   *
   * Returns the clone, or null when the user cancelled the destination picker —
   * which is a decision, not a failure, so no error is left behind.
   */
  async function clone(url?: string): Promise<RepoInfo | null> {
    const target = (url ?? cloneUrl.value).trim();
    if (!target) return null;
    const info = await store.clone(target);
    if (info) cloneUrl.value = "";
    return info;
  }

  /**
   * Ask for a folder and drop it into the path field.
   *
   * Returns quietly when the user cancels: the picker being dismissed is not
   * an error, and the path they had typed before opening it is left alone.
   */
  async function browse(): Promise<void> {
    const picked = await commands.pickDirectory();
    if (picked) pathInput.value = picked;
  }

  function select(id: string): void {
    store.select(id);
  }

  /** Remove a repository from the recent list (it is closed, not deleted on disk). */
  function remove(id: string): void {
    store.remove(id);
  }

  /** Reveal a repository's working directory in the OS file manager. */
  async function reveal(id: string): Promise<ErrorPayload | null> {
    return store.reveal(id);
  }

  const nearby = computed(() => store.nearby);
  const nearbyState = computed(() => store.nearbyState);

  return {
    pathInput,
    cloneUrl,
    nearby,
    nearbyState,
    repositories,
    selectedId,
    selected,
    busy,
    lastError,
    remotes,
    defaultRemote,
    open,
    init,
    clone,
    browse,
    discoverNearby: store.discoverNearby,
    select,
    remove,
    reveal,
  };
}
