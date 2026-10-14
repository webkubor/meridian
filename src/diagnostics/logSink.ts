/**
 * The local log sink for the failure net.
 *
 * A function rather than an import-time call because `resetFailureNet()` (the
 * flow-test harness, and any future reset) drops the configuration; the app
 * calls this at boot and the harness calls it after every reset, so "a failure
 * reaches the log" is covered end to end in the integrated flow suite instead
 * of only in unit tests.
 *
 * The line starts with `command=` and `code=` so a report's toast code can be
 * searched for in the log: the backend's own outcome line uses the same
 * vocabulary. Writing the log is best-effort and never rejects into the UI — a
 * failure to log is dropped rather than shown as a second, confusing toast.
 *
 * It lives here rather than in `bridges/commands.ts` because that module's
 * exports are the command wrappers, and the IPC contract gate treats a new
 * export as a command that needs a wire sample.
 */

import { diagnosticsLogWrite } from "../bridges/commands";
import { configureFailureNet } from "./failures";

export function installFailureLog(): void {
  configureFailureNet({
    log: ({ command, error }) => {
      void diagnosticsLogWrite("error", `command=${command} code=${error.code} ${JSON.stringify(error)}`, command).catch(() => undefined);
    },
  });
}
