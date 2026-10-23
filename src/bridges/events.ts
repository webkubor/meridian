/** Subscription helper for the Rust-side event bus (`events.rs`). */

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AppEvent } from "./bindings";

import { onMockAppEvent } from "./mock/index";

/** The window event name every `AppEvent` travels on (mirrors `events.rs`). */
export const APP_EVENT = "meridian://app-event";

export function onAppEvent(handler: (event: AppEvent) => void): Promise<UnlistenFn> {
  if (import.meta.env.MODE === "mock") {
    // In-memory bus from the mock state layer; handlers stay synchronous.
    const unlisten = onMockAppEvent(handler);
    return Promise.resolve(unlisten as UnlistenFn);
  }
  return listen<AppEvent>(APP_EVENT, (event) => handler(event.payload));
}
