import type { ErrorPayload } from "../bridges/bindings";
import { reportFailure } from "../diagnostics/failures";

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

/**
 * The error arm for `unknown`, whatever was thrown.
 *
 * `reportFailure` records and logs the failure (so it is in Diagnostics even
 * when it came from a mapper rather than the backend) and returns a well-formed
 * payload; it deliberately does *not* mark the failure handled. The component
 * that renders this state claims it (`useFailureClaim`), and if that component
 * is not on screen — the pane is on another tab — the failure still reaches the
 * user as a fallback toast. `source` is the command or view-model site, which is
 * what the report quotes when the failure never had a command name.
 */
export function errorState(error: unknown, source = "list.error"): LoadState<never> {
  return { status: "error", error: reportFailure(error, source) };
}
