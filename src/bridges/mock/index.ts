/**
 * Entry point for browser mock mode (`vite --mode mock`).
 *
 * Imported only from `src/bridges/commands.ts`, `src/bridges/events.ts` and
 * `src/bridges/external.ts` behind `import.meta.env.MODE === "mock"` guards;
 * production bundles and vitest runs tree-shake all of this away. Nothing in
 * this directory may import `@tauri-apps/*` (networkGuard whitelist).
 */

import type { AppEvent } from "../bindings";

import { mockInvoke } from "./invoke";
import { emitMockEvent, onMockEvent, resetMockState, MOCK_REPO_ID } from "./state";

export { mockInvoke };

/** Subscribe to the mock app-event bus (same frames the Tauri backend emits). */
export function onMockAppEvent(handler: (event: AppEvent) => void): () => void {
  return onMockEvent(handler);
}

/** Drive the app-event bus by hand — lets fixtures push UI refreshes. */
export function emitMockAppEvent(event: AppEvent): void {
  emitMockEvent(event);
}

export { resetMockState, MOCK_REPO_ID };
