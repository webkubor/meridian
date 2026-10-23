/**
 * Opening a link in the user's real browser.
 *
 * A Tauri webview has no browser chrome of its own: `window.open` would either
 * be swallowed or spawn a second frameless window with no address bar — which is
 * a phishing shape, since the user cannot see where they are going. The opener
 * plugin hands the URL to the system instead, where the address is visible and
 * the user's own browser rules apply.
 *
 * This is the only module that imports the opener plugin, so the app has exactly
 * one place that can hand a URL to the operating system. Callers pass URLs the
 * backend built; nothing here interprets one.
 */

import { openUrl } from "@tauri-apps/plugin-opener";

import { claimFailure } from "../diagnostics/failures";
import { i18n } from "../i18n";

export interface OpenResult {
  /** Whether a browser was actually asked to open the link. */
  opened: boolean;
  /** Why not, in the user's language; the technical reason is in Diagnostics. */
  reason?: string;
}

/**
 * Opens `url` in the default browser.
 *
 * Refuses anything that is not `http(s)` or `mailto`. The backend composes the
 * URLs this app opens, so the check should never fire — which is the point: if
 * a future change lets a remote string reach here, the failure is a refused link
 * rather than a `file:` or `javascript:` URL handed to the system.
 *
 * A refusal (no browser handler, plugin error) used to be a `false` that every
 * caller ignored, so "Report a problem" did nothing at all and said nothing.
 * The attempt is now recorded through the failure net — claimed, because the
 * caller decides what the user sees (a danger toast, a copyable URL) — and the
 * caller gets the result. Never throw: the URL is in the caller's hands, and a
 * thrown error would make the report path itself throw.
 */
export async function openExternal(url: string): Promise<OpenResult> {
  const trimmed = url.trim();
  const failed = (technical: string): OpenResult => {
    claimFailure(
      { code: "unclassified", message: i18n.global.t("shell.toast.linkFailed"), detail: technical.slice(0, 200), retryable: true, domain: "internal" },
      "opener.openUrl",
    );
    return { opened: false, reason: i18n.global.t("shell.toast.linkFailed") };
  };
  if (!/^(https?:|mailto:)/i.test(trimmed)) return failed("refused: not an http(s) or mailto URL");
  if (import.meta.env.MODE === "mock") {
    // Browser mock mode has no system opener; log and report success so the
    // flow (feedback links, reveal-in-finder toasts) stays exercisable.
    console.info(`[mock] openExternal: ${trimmed}`);
    return { opened: true };
  }
  try {
    await openUrl(trimmed);
    return { opened: true };
  } catch (error) {
    return failed(error instanceof Error ? error.message : String(error));
  }
}

/** Whether a string is safe to put in an anchor's `href`. */
export function isExternalLink(url: string): boolean {
  return /^(https?:|mailto:)/i.test(url.trim());
}
