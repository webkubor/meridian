/**
 * The commit list's row model: display rows (date groups, commits, skeletons),
 * their heights and offsets, and the date formatting the rows show.
 *
 * Display rows are derived from the flat `commits` array the history engine
 * returns, so the graph layout's row space (indices into `commits`) maps 1:1
 * onto the rows that carry a commit — `commitIndex` is that mapping.
 *
 * Copy comes from the catalogue through the i18n runtime rather than through a
 * `t` argument: `buildDisplayRows` is called from a computed inside
 * `VirtualCommitList.vue`, and reading `i18n.global.t` inside that computed is
 * what makes the rows re-label themselves when the user switches language.
 * Passing `t` in by hand would put that burden on every caller and still need
 * each one to remember it.
 */

import type { CommitInfo } from "../../bridges/bindings";
import { currentLocale, i18n } from "../../i18n";
import type { BadgeStrip, GraphDensity, GraphLaneSegment } from "../graph";

export type DateGrouping = "off" | "day" | "week" | "month";

export type CommitRowKind = "commit" | "merge" | "forkPoint" | "dateGroup" | "skeleton";

/**
 * Bubble lane per commit, parallel to the `commits` array. This is the shape the
 * UI layer's shared `contracts.ts` carries (`GraphRow[]`, one entry per commit
 * row), so the history pane can feed bubble lanes to the list when its
 * `GraphLayout` has no `lanes` array of its own. `lane: null` (a row that is not
 * a commit) is treated as "no bubble".
 */
export interface CommitGraphRowLane {
  lane: number | null;
}

export interface CommitDisplayRow {
  /** Stable identity across paging: `c:<oid>`, `g:<period>`, `s:<n>`. */
  key: string;
  kind: CommitRowKind;
  /** Index into the `commits` array, or null for group/skeleton rows. */
  commitIndex: number | null;
  height: number;
  /** The date-group label this row belongs to (null when grouping is off). */
  group: string | null;
  /** The row's own group label, for `dateGroup` rows. */
  label: string | null;
  commit: CommitInfo | null;
}

/**
 * Row heights per density, mirroring `--md-row-compact` / `--md-row-regular`
 * (docs/04 §2.3). Date-group rows are a fixed compact strip.
 */
export const ROW_HEIGHT: Record<GraphDensity, number> = { compact: 24, regular: 28 };
export const GROUP_ROW_HEIGHT = 24;

/** The per-commit graph payload a row paints from (built and cached by the list). */
export interface RowGraph {
  /** This commit's layout row (= index into `commits`/`GraphLayout.lanes`). */
  layoutRow: number;
  lane: number;
  segments: readonly GraphLaneSegment[];
  /** Content-space centre y of every layout row. */
  centers: Float64Array;
  strip: BadgeStrip | null;
  /** x where the badge strip begins (the graph gutter). */
  stripOriginX: number;
  /** Left inset for the row's text (graph gutter + badge strip + gap). */
  inset: number;
  continuationTop: readonly number[];
  continuationBottom: readonly number[];
  isHead: boolean;
}

export interface BuildRowsOptions {
  density: GraphDensity;
  dateGrouping: DateGrouping;
  /** Oids the engine reported as fork points (`history_fork_points_load`). */
  forkPoints?: readonly string[];
  /** Trailing skeleton rows (first load / paging). */
  skeletons?: number;
  /** Injectable "now" in seconds, for deterministic labels. */
  now?: number;
}

export function buildDisplayRows(
  commits: readonly CommitInfo[],
  options: BuildRowsOptions,
): CommitDisplayRow[] {
  const rows: CommitDisplayRow[] = [];
  const rowHeight = ROW_HEIGHT[options.density];
  const forks = options.forkPoints && options.forkPoints.length > 0 ? new Set(options.forkPoints) : null;
  const now = options.now ?? Date.now() / 1000;

  let groupKey: string | null = null;
  let groupLabel: string | null = null;

  commits.forEach((commit, index) => {
    if (options.dateGrouping !== "off") {
      const key = groupKeyFor(commit.committer.time, options.dateGrouping);
      if (key !== groupKey) {
        groupKey = key;
        groupLabel = formatGroupLabel(commit.committer.time, options.dateGrouping, now);
        rows.push({
          key: `g:${key}`,
          kind: "dateGroup",
          commitIndex: null,
          height: GROUP_ROW_HEIGHT,
          group: groupLabel,
          label: groupLabel,
          commit: null,
        });
      }
    }
    const kind: CommitRowKind = forks?.has(commit.oid)
      ? "forkPoint"
      : commit.parents.length > 1
        ? "merge"
        : "commit";
    rows.push({
      key: `c:${commit.oid}`,
      kind,
      commitIndex: index,
      height: rowHeight,
      group: groupLabel,
      label: null,
      commit,
    });
  });

  const skeletons = Math.max(0, options.skeletons ?? 0);
  for (let i = 0; i < skeletons; i += 1) {
    rows.push({
      key: `s:${i}`,
      kind: "skeleton",
      commitIndex: null,
      height: rowHeight,
      group: groupLabel,
      label: null,
      commit: null,
    });
  }

  return rows;
}

/** Prefix-sum offsets: `offsets[i]` is row `i`'s top, length `rows + 1`. */
export function computeOffsets(rows: readonly CommitDisplayRow[]): Float64Array {
  const offsets = new Float64Array(rows.length + 1);
  let y = 0;
  for (let i = 0; i < rows.length; i += 1) {
    offsets[i] = y;
    y += rows[i].height;
  }
  offsets[rows.length] = y;
  return offsets;
}

/** Index of the row containing content-space y `value` (binary search). */
export function findRowAt(offsets: Float64Array, value: number): number {
  const count = offsets.length - 1;
  if (count <= 0) return 0;
  const target = Math.max(0, value);
  let lo = 0;
  let hi = count - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (offsets[mid] <= target) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

export function isCommitRow(row: CommitDisplayRow): boolean {
  return row.commitIndex != null && row.commit != null;
}

// --- Date formatting -------------------------------------------------------

/**
 * Dates are formatted for the *app's* language, not the system's. The app
 * ships Chinese by default whatever the Mac is set to, and a commit list that
 * mixes Chinese copy with English month names reads as half-translated.
 *
 * The formatters are cached per locale because constructing an
 * `Intl.DateTimeFormat` is the expensive part, and the locale only changes
 * when the user switches language.
 */
const DATE_FORMATS = {
  weekdayDate: { weekday: "long", day: "numeric", month: "long", year: "numeric" },
  dayMonthYear: { day: "numeric", month: "short", year: "numeric" },
  dayMonth: { day: "numeric", month: "short" },
  monthYear: { month: "long", year: "numeric" },
  timeOfDay: { hour: "2-digit", minute: "2-digit" },
} as const satisfies Record<string, Intl.DateTimeFormatOptions>;

type DateFormat = keyof typeof DATE_FORMATS;

const formatters = new Map<string, Intl.DateTimeFormat>();

function formatter(kind: DateFormat): Intl.DateTimeFormat {
  const locale = currentLocale();
  const key = `${locale}|${kind}`;
  const cached = formatters.get(key);
  if (cached) return cached;
  const built = new Intl.DateTimeFormat(locale, DATE_FORMATS[kind]);
  formatters.set(key, built);
  return built;
}

/** Period bucket a timestamp falls into, per grouping mode (local time). */
export function groupKeyFor(seconds: number, mode: DateGrouping): string {
  const date = new Date(seconds * 1000);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  if (mode === "month") return `${year}-${month}`;
  if (mode === "week") {
    const start = weekStart(date);
    return `w:${start.getFullYear()}-${String(start.getMonth() + 1).padStart(2, "0")}-${String(start.getDate()).padStart(2, "0")}`;
  }
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function formatGroupLabel(seconds: number, mode: DateGrouping, nowSeconds: number): string {
  const date = new Date(seconds * 1000);
  if (mode === "month") return formatter("monthYear").format(date);
  if (mode === "week") {
    return i18n.global.t("commitList.weekOf", {
      date: formatter("dayMonth").format(weekStart(date)),
    });
  }
  const now = new Date(nowSeconds * 1000);
  if (isSameDay(date, now)) return i18n.global.t("commitList.today");
  const yesterday = new Date(now.getTime() - 24 * 60 * 60 * 1000);
  if (isSameDay(date, yesterday)) return i18n.global.t("commitList.yesterday");
  return formatter("weekdayDate").format(date);
}

/** The row's date column: time for today, day+month for this year, full date otherwise. */
export function formatWhen(seconds: number, nowSeconds: number): string {
  const date = new Date(seconds * 1000);
  const now = new Date(nowSeconds * 1000);
  if (isSameDay(date, now)) return formatter("timeOfDay").format(date);
  if (date.getFullYear() === now.getFullYear()) return formatter("dayMonth").format(date);
  return formatter("dayMonthYear").format(date);
}

/** Exact timestamp for tooltips/aria labels. */
export function formatExact(seconds: number): string {
  const date = new Date(seconds * 1000);
  return `${formatter("dayMonthYear").format(date)}, ${formatter("timeOfDay").format(date)}`;
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
  );
}

function weekStart(date: Date): Date {
  const start = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const weekday = (start.getDay() + 6) % 7; // Monday = 0
  start.setDate(start.getDate() - weekday);
  return start;
}
