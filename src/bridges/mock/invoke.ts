/**
 * The mock replacement for `tauriInvoke`.
 *
 * Resolves through the handler registry in `handlers.ts`; unknown commands
 * throw a loud, greppable error so a new command without a fixture fails
 * fast in the browser instead of silently returning nothing. A tiny artificial
 * delay keeps loading states observable during UI iteration.
 *
 * `call()` forwards the wrapper payload verbatim: most commands travel as
 * `{ args: {...} }` (Tauri maps that onto the command's argument struct), but
 * `repository_clone_suggest_destination` sends `{ url }` and
 * `app_update_install` sends `{ onProgress }` unwrapped. Handlers see the
 * inner argument object either way.
 */

import { handlers } from "./handlers";

const DELAY_MS = 40;

function unwrap(args?: Record<string, unknown>): Record<string, unknown> {
  if (!args) return {};
  const inner = args.args;
  if (
    args.url === undefined &&
    args.onProgress === undefined &&
    typeof inner === "object" &&
    inner !== null
  ) {
    return inner as Record<string, unknown>;
  }
  return args;
}

export async function mockInvoke<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  const handler = handlers[name];
  if (!handler) {
    throw new Error(
      `[mock] No fixture handler for command "${name}". Add one in src/bridges/mock/handlers.ts.`,
    );
  }
  if (DELAY_MS > 0) {
    await new Promise<void>((resolve) => setTimeout(resolve, DELAY_MS));
  }
  return handler(unwrap(args)) as T;
}
