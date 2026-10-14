import type { ErrorPayload } from "../bridges/bindings";

/**
 * The four states every asynchronous list renders (docs/04 §1.2):
 * `loading | empty | error | data`. Making it a type means no list can forget
 * a state — the stores own it, the components render all arms.
 */
export type LoadState<T> =
  | { status: "loading" }
  | { status: "empty" }
  | { status: "error"; error: ErrorPayload }
  | { status: "data"; data: T };

export function dataState<T>(data: T[]): LoadState<T[]> {
  return data.length > 0 ? { status: "data", data } : { status: "empty" };
}

export function errorState(error: unknown): LoadState<never> {
  return { status: "error", error: error as ErrorPayload };
}
