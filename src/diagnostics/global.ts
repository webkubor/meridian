import type { App } from "vue";

import { captureFailure } from "./failures";
import { i18n } from "../i18n";
import { normalizeError } from "./failures";

/** Re-entrancy guard: the handler's own failures must not feed the handler again. */
let reporting = false;

function payloadFor(value: unknown, source: string) {
  const normalized = normalizeError(value);
  const message = i18n.global.t("diagnostics.internalError");
  return {
    ...normalized,
    code: normalized.code,
    message,
    detail: [source, normalized.detail ?? normalized.message].filter(Boolean).join(": "),
    domain: normalized.domain,
  };
}

export function captureGlobalError(value: unknown, source: string): void {
  if (reporting) return;
  reporting = true;
  try {
    captureFailure(value, `internal:${source}`, { payload: payloadFor(value, source) });
  } catch {
    // The end of the failure path: it must never throw or reject.
  } finally {
    reporting = false;
  }
}

/**
 * Window `error` events that are browser notices rather than failures. The
 * ResizeObserver loop notice fires whenever a virtualised list re-measures
 * itself in the same frame; it changes nothing and has no error object. Turning
 * it into a persistent danger toast trains people to ignore the toast the real
 * failures use.
 */
const BENIGN_WINDOW_ERRORS = [/ResizeObserver loop (?:limit exceeded|completed with undelivered notifications)/i];

export function isBenignWindowError(message: unknown): boolean {
  return typeof message === "string" && BENIGN_WINDOW_ERRORS.some((pattern) => pattern.test(message));
}

export function installGlobalErrorHandlers(app: App): () => void {
  app.config.errorHandler = (error, _instance, info) => captureGlobalError(error, `vue:${info}`);
  const onError = (event: ErrorEvent): void => {
    if (!event.error && isBenignWindowError(event.message)) {
      console.debug(`[meridian] ignored a benign window notice: ${event.message}`);
      return;
    }
    captureGlobalError(event.error ?? event.message, "window.error");
  };
  const onRejection = (event: PromiseRejectionEvent): void => captureGlobalError(event.reason, "window.unhandledrejection");
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
  };
}