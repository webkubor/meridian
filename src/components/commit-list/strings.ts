/**
 * Commit-list copy — a `t(...)` call site, not a second place copy is written.
 *
 * Every string here reads `commitList.*` from the catalogue. This module must
 * stay free of `vue` and `vue-i18n` so the row model beside it remains
 * testable on its own, so the lookup is injected: every exported entry takes a
 * `t`, and the component passes the one `useI18n()` gives it. That also keeps
 * the strings resolving inside render/computed context, so switching language
 * re-labels the list without a reload.
 *
 * Left alone on purpose:
 *  - `groupLabel` / `refKindLabel` return data the caller already holds (a
 *    period label, a ref kind), not prose. There is nothing to translate.
 *  - `overflowBadge` used to be a literal here; it is a catalogue key now, and
 *    identical in zh-CN by design — a count badge is `+12` in every language.
 */

/** Structural type of vue-i18n's `t`, narrowed to what these strings use. */
export type ComposerTranslation = (key: string, named?: Record<string, unknown>) => string;

export function commitListStrings(t: ComposerTranslation) {
  return {
    listLabel: t("commitList.listLabel"),
    listHint: t("commitList.listHint"),
    today: t("commitList.today"),
    yesterday: t("commitList.yesterday"),
    weekOf: (date: string): string => t("commitList.weekOf", { date }),
    rowLabel: (summary: string, author: string, when: string): string =>
      t("commitList.rowLabel", { summary, author, when }),
    /** Label of the collapsed "… +N commits" chip in the graph gutter. */
    overflowBadge: (dropped: number): string => t("commitList.overflowBadge", { count: dropped }),
    skeletonsLabel: t("commitList.skeletonsLabel"),
    skeletonAria: t("commitList.skeletonAria"),
  } as const;
}

/** Pass-throughs: the caller already holds the final label. */
export const groupLabel = (label: string): string => label;
export const refKindLabel = (kind: string): string => kind;
