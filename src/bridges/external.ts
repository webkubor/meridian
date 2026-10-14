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

/**
 * Opens `url` in the default browser.
 *
 * Refuses anything that is not `http(s)` or `mailto`. The backend composes the
 * URLs this app opens, so the check should never fire — which is the point: if a
 * future change lets a remote string reach here, the failure is a refused link
 * rather than a `file:` or `javascript:` URL handed to the system.
 *
 * Resolves to whether a browser was asked to open it; a false return is not an
 * error worth a modal, but the caller may want to offer the URL as text.
 */
export async function openExternal(url: string): Promise<boolean> {
  const trimmed = url.trim();
  if (!/^(https?:|mailto:)/i.test(trimmed)) return false;
  try {
    await openUrl(trimmed);
    return true;
  } catch {
    // No registered handler, or the plugin refused it.
    return false;
  }
}

/** Whether a string is safe to put in an anchor's `href`. */
export function isExternalLink(url: string): boolean {
  return /^(https?:|mailto:)/i.test(url.trim());
}
