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

export function installGlobalErrorHandlers(app: App): () => void {
  app.config.errorHandler = (error, _instance, info) => captureGlobalError(error, `vue:${info}`);
  const onError = (event: ErrorEvent): void => captureGlobalError(event.error ?? event.message, "window.error");
  const onRejection = (event: PromiseRejectionEvent): void => captureGlobalError(event.reason, "window.unhandledrejection");
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
  };
}