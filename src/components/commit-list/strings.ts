/**
 * Commit-list copy (docs/04 §10: no inline literals in components).
 *
 * This is the commit-list slice of the message catalogue. When the app-wide
 * catalogue lands these entries move into it unchanged — the keys are chosen to
 * be catalogue keys already (`commitList.*`).
 */

export const commitListStrings = {
  listLabel: "Commit history",
  listHint: "Use the arrow keys to move the selection.",
  today: "Today",
  yesterday: "Yesterday",
  weekOf: (date: string): string => `Week of ${date}`,
  rowLabel: (summary: string, author: string, when: string): string =>
    `${summary} — ${author}, ${when}`,
  groupLabel: (label: string): string => label,
  overflowBadge: (dropped: number): string => `+${dropped}`,
  forkPointBadge: "fork point",
  refKindLabel: (kind: string): string => kind,
  skeletonsLabel: "Loading commits",
  skeletonAria: "Loading",
} as const;
