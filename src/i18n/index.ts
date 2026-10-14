/**
 * i18n runtime.
 *
 * Owner constraint: the app ships **Simplified Chinese and English, Chinese by
 * default**, switchable in Settings, and the choice is remembered.
 *
 * Why this file exists at all rather than reading `navigator.language`: the
 * product decision is that a new install opens in Chinese regardless of the
 * system language, so the default is a constant, not a detection. Detection
 * would make the first-run experience depend on a machine setting the user
 * never chose for this app.
 *
 * The backend does not localise. `ErrorPayload.message` is an English sentence
 * built in Rust and is treated here as a developer-facing fallback only; every
 * code has an entry in `errors.byCode`, so the fallback should never be seen.
 */

import { createI18n } from "vue-i18n";

import type { ErrorCode } from "../bridges/bindings";
import en, { type Catalog } from "./locales/en";
import zhCN from "./locales/zh-CN";

export const LOCALES = ["zh-CN", "en"] as const;
export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "zh-CN";
export const FALLBACK_LOCALE: Locale = "en";

const STORAGE_KEY = "meridian.locale";

export function isLocale(value: unknown): value is Locale {
  return typeof value === "string" && (LOCALES as readonly string[]).includes(value);
}

function readStoredLocale(): Locale | null {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    return isLocale(raw) ? raw : null;
  } catch {
    // Private mode or a locked-down webview: fall back to the default rather
    // than failing to start.
    return null;
  }
}

export const i18n = createI18n<[Catalog], Locale, false>({
  legacy: false,
  globalInjection: true,
  locale: readStoredLocale() ?? DEFAULT_LOCALE,
  fallbackLocale: FALLBACK_LOCALE,
  messages: { "zh-CN": zhCN, en },
  // An untranslated key should be loud in development and harmless in release.
  missingWarn: import.meta.env.DEV,
  fallbackWarn: import.meta.env.DEV,
});

export function currentLocale(): Locale {
  const value = i18n.global.locale.value;
  return isLocale(value) ? value : DEFAULT_LOCALE;
}

export function setLocale(next: Locale): void {
  i18n.global.locale.value = next;
  try {
    window.localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // Not being able to remember the choice is not a reason to refuse it.
  }
  document.documentElement.lang = next;
}

/** `en` only if the requested locale is not there; never a bare string. */
export function localeLabel(locale: Locale): string {
  return i18n.global.t(`language.${locale === "zh-CN" ? "zhCN" : "en"}`);
}

// --- formatting ------------------------------------------------------------
// Dates, times and byte counts change shape with the language, so they go
// through the active locale rather than being built from English words.

function intlLocale(): string {
  return currentLocale();
}

export function formatDate(value: string | number | Date): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return new Intl.DateTimeFormat(intlLocale(), {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(date);
}

export function formatDateTime(value: string | number | Date): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return new Intl.DateTimeFormat(intlLocale(), {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

/** "3 days ago" / "3 天前". */
export function formatRelative(value: string | number | Date, now = Date.now()): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);

  const deltaSeconds = Math.round((date.getTime() - now) / 1000);
  const units: Array<[Intl.RelativeTimeFormatUnit, number]> = [
    ["year", 60 * 60 * 24 * 365],
    ["month", 60 * 60 * 24 * 30],
    ["week", 60 * 60 * 24 * 7],
    ["day", 60 * 60 * 24],
    ["hour", 60 * 60],
    ["minute", 60],
  ];

  const formatter = new Intl.RelativeTimeFormat(intlLocale(), { numeric: "auto" });
  for (const [unit, seconds] of units) {
    if (Math.abs(deltaSeconds) >= seconds) {
      return formatter.format(Math.round(deltaSeconds / seconds), unit);
    }
  }
  return formatter.format(deltaSeconds, "second");
}

/** Byte counts with the locale's own units (KB/MB vs 千字节). */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  const units = ["B", "kB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return `${new Intl.NumberFormat(intlLocale(), {
    maximumFractionDigits: value < 10 && unit > 0 ? 1 : 0,
  }).format(value)} ${units[unit]}`;
}

/** Locale-aware integer, e.g. 1,204 / 1,204. */
export function formatCount(value: number): string {
  return new Intl.NumberFormat(intlLocale()).format(value);
}

// --- errors ----------------------------------------------------------------

/**
 * Localised text for an error envelope.
 *
 * Looks the code up in the catalogue and interpolates `params`. The backend's
 * `message` is only used when a code has no entry — which, because
 * `errors.byCode` is typed against `ErrorCode`, cannot happen without failing
 * the build.
 */
export function errorText(payload: {
  code: ErrorCode;
  message?: string;
  params?: Record<string, string>;
}): string {
  const key = `errors.byCode.${payload.code}`;
  const translated = i18n.global.t(key, payload.params ?? {});
  // vue-i18n returns the key itself when it is missing.
  return translated === key ? (payload.message ?? i18n.global.t("errors.byCode.unclassified")) : translated;
}

export default i18n;
