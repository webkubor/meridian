/**
 * The renderer's side of the failure net.
 *
 * A store that catches a failure keeps it for display (`reportFailure`) but
 * cannot know whether anything that shows it is on screen. The component that
 * *does* render the error calls `useFailureClaim`: while the component exists
 * and the error is in its props, the fallback toast stays quiet; if the
 * component is not mounted (a pane on another tab), nothing claims the failure
 * and the net shows the toast after its grace period.
 */

import { watch, type InjectionKey, type WatchSource } from "vue";

import { markHandled } from "./failures";

export function useFailureClaim(source: WatchSource<unknown>): void {
  // `sync`: the claim must land before the net's timer can fire, and a watcher
  // that waits for the next flush would race the grace period under fake timers.
  watch(
    source,
    (error) => {
      if (error) markHandled(error);
    },
    { immediate: true, flush: "sync" },
  );
}

/**
 * How a presentational component sends the user to the Diagnostics screen
 * (optionally focused on one failure). Provided once by `App.vue`; components
 * that render an error inject it instead of importing the router, so they stay
 * props-and-events components.
 */
export type OpenDiagnostics = (failureId?: string) => void;
export const OPEN_DIAGNOSTICS: InjectionKey<OpenDiagnostics> = Symbol("meridian.openDiagnostics");
