<script setup lang="ts">
/**
 * The virtualised commit list (docs/04 §S8, §6 `<VirtualList>`).
 *
 * Rows are absolutely positioned over a spacer whose height is the loaded
 * row total; only the visible window (plus overscan) is in the DOM, so tens of
 * thousands of rows cost the same as a screenful. The list owns:
 *   - row model: `buildDisplayRows` (date groups, fork-point/merge kinds,
 *     skeletons) with prefix-sum offsets for O(log n) hit testing;
 *   - the graph payload per commit: lane, crossing strokes (from the history
 *     engine's `GraphLayout`), badge strip and text inset — memoised so a row
 *     that keeps its content is never repainted while scrolling;
 *   - scroll anchoring: the first visible row is tracked continuously and
 *     restored after paging appends, density changes and resizes, so the
 *     viewport never jumps;
 *   - incremental paging: `need-more` fires when the window approaches the
 *     loaded tail;
 *   - theme changes: the canvas theme is re-read from the tokens and rows
 *     repaint in place (no scroll write).
 *
 * The list renders no states of its own beyond the row kinds: loading / empty /
 * error / large-warning chrome belongs to `<ListShell>` (docs/04 §1.2), which
 * wraps this component in the history pane.
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

import type { CommitInfo } from "../../bridges/bindings";
import {
  buildSegmentIndex,
  continuationLanes,
  gutterWidth,
  hasHeadDecoration,
  laneColor,
  laneCountOf,
  layoutBadgeStrip,
  metricsFor,
  parseDecorations,
  segmentIdsAt,
  useGraphTheme,
  type BadgeStrip,
  type GraphDensity,
  type GraphLaneSegment,
  type GraphLayout,
  type GraphMetrics,
  type SegmentIndex,
} from "../graph";
import CommitRow from "./CommitRow.vue";
import {
  buildDisplayRows,
  computeOffsets,
  findRowAt,
  ROW_HEIGHT,
  type CommitDisplayRow,
  type DateGrouping,
  type RowGraph,
} from "./rows";
import { commitListStrings } from "./strings";
import type { CommitGraphRowLane } from "./rows";

const props = withDefaults(
  defineProps<{
    /** The loaded page(s) of commits, newest first (the engine's order). */
    commits: readonly CommitInfo[];
    /**
     * The engine's lane assignment for exactly those commits (docs/03 §5).
     * `segments` is required; bubble lanes come from `layout.lanes` or, when
     * that is absent, from the `graphRows` prop.
     */
    layout: GraphLayout | null;
    /**
     * Bubble lane per commit, parallel to `commits` (indices must match — it is
     * NOT the pane's own display-row array if that interleaves date-group rows).
     * Used only when `layout.lanes` is absent.
     */
    graphRows?: readonly CommitGraphRowLane[] | null;
    density?: GraphDensity;
    /** Date-group headers (Tower-style); "off" renders a flat list. */
    dateGrouping?: DateGrouping;
    selectedOid?: string | null;
    /** Repo HEAD commit — draws the head decoration even without a decoration string. */
    headOid?: string | null;
    /** Fork-point oids (`history_fork_points_load`). */
    forkPoints?: readonly string[];
    /** Known remote names, used to type bare "origin/x" decorations. */
    remotes?: readonly string[];
    /** View setting: graph column on/off (docs/04 §S8). */
    showGraph?: boolean;
    /** First load: renders skeletons. */
    loading?: boolean;
    /** Paging in progress: renders tail skeletons. */
    loadingMore?: boolean;
    /** The engine reported more commits (`CommitPage.truncated`). */
    hasMore?: boolean;
    /** The window starts mid-history (compare mode): draws top continuation nibs. */
    hasMoreAbove?: boolean;
    /** Rows rendered beyond the viewport, each side. */
    overscan?: number;
    /** Distance from the loaded tail (in rows) at which `need-more` fires. */
    pageThreshold?: number;
    /** Max width of the ref badge strip. */
    badgeStripMax?: number;
    /** Max width of one badge chip. */
    badgeChipMax?: number;
  }>(),
  {
    density: "regular",
    dateGrouping: "day",
    graphRows: null,
    selectedOid: null,
    headOid: null,
    forkPoints: () => [],
    remotes: () => [],
    showGraph: true,
    loading: false,
    loadingMore: false,
    hasMore: false,
    hasMoreAbove: false,
    overscan: 10,
    pageThreshold: 80,
    badgeStripMax: 260,
    badgeChipMax: 160,
  },
);

const emit = defineEmits<{
  select: [commit: CommitInfo];
  activate: [commit: CommitInfo];
  hover: [commit: CommitInfo | null];
  "ref-click": [payload: { commit: CommitInfo; ref: string; label: string }];
  "need-more": [];
  "reach-end": [];
}>();

const theme = useGraphTheme();
const scroller = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewport = ref({ width: 0, height: 0 });
const hoveredKey = ref<string | null>(null);

const EMPTY_SEGMENTS: readonly GraphLaneSegment[] = Object.freeze([]);
const EMPTY_LANES: readonly number[] = Object.freeze([]);
const BADGE_TEXT_GAP = 8;

// --- Row model -------------------------------------------------------------

const rows = computed<CommitDisplayRow[]>(() =>
  buildDisplayRows(props.commits, {
    density: props.density,
    dateGrouping: props.dateGrouping,
    forkPoints: props.forkPoints,
    skeletons: props.loading ? 6 : props.loadingMore ? 3 : 0,
  }),
);

const offsets = computed(() => computeOffsets(rows.value));
const totalHeight = computed(() => offsets.value[rows.value.length] ?? 0);
const rowHeight = computed(() => ROW_HEIGHT[props.density]);

const keyIndex = computed(() => {
  const index = new Map<string, number>();
  rows.value.forEach((row, i) => index.set(row.key, i));
  return index;
});

// --- Graph model -----------------------------------------------------------

const metrics = computed<GraphMetrics>(() => metricsFor(props.density, theme.value));

const segmentIndex = computed<SegmentIndex | null>(() =>
  props.layout ? buildSegmentIndex(props.layout.segments, props.commits.length) : null,
);

const laneCount = computed(() =>
  laneCountOf(props.layout, props.graphRows?.map((row) => row.lane ?? -1)),
);

/** Bubble lane for a commit index: `layout.lanes` first, else the `graphRows` prop. */
function laneFor(commitIndex: number): number {
  const fromLayout = props.layout?.lanes?.[commitIndex];
  if (fromLayout != null && Number.isFinite(fromLayout)) return fromLayout;
  const fromRows = props.graphRows?.[commitIndex]?.lane;
  return fromRows != null && Number.isFinite(fromRows) ? fromRows : -1;
}

const gutter = computed(() => {
  const full = gutterWidth(laneCount.value, metrics.value);
  if (full <= 0) return 0;
  const cap = Math.max(96, viewport.value.width * 0.35);
  return Math.min(full, cap);
});

/** Content-space centre y per layout row (= index into `commits`). */
const centers = computed<Float64Array>(() => {
  const out = new Float64Array(props.commits.length);
  const list = rows.value;
  const off = offsets.value;
  for (let i = 0; i < list.length; i += 1) {
    const row = list[i];
    if (row.commitIndex != null) out[row.commitIndex] = off[i] + row.height / 2;
  }
  return out;
});

const tailLanes = computed<readonly number[]>(() => {
  const layout = props.layout;
  if (!layout || !props.hasMore || props.commits.length === 0) return EMPTY_LANES;
  return continuationLanes(layout, props.commits.length - 1, "bottom", laneFor(props.commits.length - 1));
});

const headLanes = computed<readonly number[]>(() => {
  const layout = props.layout;
  if (!layout || !props.hasMoreAbove || props.commits.length === 0) return EMPTY_LANES;
  return continuationLanes(layout, 0, "top", laneFor(0));
});

// --- Per-row graph payloads (memoised so scrolling does not repaint) -------

let payloadCache = new Map<number, RowGraph>();
const PAYLOAD_CACHE_LIMIT = 2400;

watch(
  [
    () => props.layout,
    () => props.graphRows,
    theme,
    () => viewport.value.width,
    () => props.density,
    () => props.headOid,
    () => props.remotes,
    () => props.showGraph,
    () => props.badgeStripMax,
    () => props.badgeChipMax,
    tailLanes,
    headLanes,
  ],
  () => {
    payloadCache = new Map();
  },
);

function payloadFor(commitIndex: number): RowGraph | null {
  const cached = payloadCache.get(commitIndex);
  if (cached) return cached;
  const commit = props.commits[commitIndex];
  if (!commit) return null;
  if (payloadCache.size > PAYLOAD_CACHE_LIMIT) payloadCache = new Map();

  const index = segmentIndex.value;
  const lane = laneFor(commitIndex);

  const ids = index ? segmentIdsAt(index, commitIndex) : EMPTY_LANES;
  const segments: readonly GraphLaneSegment[] =
    ids.length === 0 || !index ? EMPTY_SEGMENTS : ids.map((id) => index.segments[id]);

  const badges = parseDecorations(commit.decorations, { remotes: props.remotes });
  const strip: BadgeStrip | null =
    badges.length === 0
      ? null
      : layoutBadgeStrip(badges, {
          theme: theme.value,
          laneColor: laneColor(theme.value, lane),
          chipHeight: props.density === "compact" ? 14 : 16,
          padX: 6,
          gap: 4,
          maxWidth: Math.max(0, Math.min(props.badgeStripMax, viewport.value.width * 0.35)),
          maxChipWidth: props.badgeChipMax,
          overflowLabel: commitListStrings.overflowBadge,
        });

  const stripOriginX = props.showGraph ? gutter.value : 0;
  const stripWidth = strip ? strip.width : 0;
  const isHead =
    hasHeadDecoration(commit.decorations) ||
    (props.headOid != null && commit.oid === props.headOid);

  const payload: RowGraph = {
    layoutRow: commitIndex,
    lane,
    segments,
    centers: centers.value,
    strip,
    stripOriginX,
    // With the graph column hidden the badge strip is not drawn either, so the
    // text must not reserve an inset for it — otherwise every subject is pushed
    // right by a gap that is not there.
    inset: props.showGraph ? stripOriginX + stripWidth + (stripWidth > 0 ? BADGE_TEXT_GAP : 0) : 0,
    continuationTop: commitIndex === 0 ? headLanes.value : EMPTY_LANES,
    continuationBottom:
      commitIndex === props.commits.length - 1 ? tailLanes.value : EMPTY_LANES,
    isHead,
  };
  payloadCache.set(commitIndex, payload);
  return payload;
}

// --- Windowing -------------------------------------------------------------

interface WindowItem {
  key: string;
  index: number;
  top: number;
  height: number;
  row: CommitDisplayRow;
  graph: RowGraph | null;
}

const range = computed(() => {
  const count = rows.value.length;
  if (count === 0) return { start: 0, end: -1 };
  const off = offsets.value;
  const first = findRowAt(off, scrollTop.value);
  const last = findRowAt(off, scrollTop.value + Math.max(0, viewport.value.height - 1));
  return {
    start: Math.max(0, first - props.overscan),
    end: Math.min(count - 1, last + props.overscan),
  };
});

const windowRows = computed<WindowItem[]>(() => {
  const { start, end } = range.value;
  const items: WindowItem[] = [];
  if (end < start) return items;
  const list = rows.value;
  const off = offsets.value;
  for (let i = start; i <= end; i += 1) {
    const row = list[i];
    items.push({
      key: row.key,
      index: i,
      top: off[i],
      height: row.height,
      row,
      graph: row.commitIndex != null ? payloadFor(row.commitIndex) : null,
    });
  }
  return items;
});

const stickyLabel = computed(() => {
  const { start, end } = range.value;
  if (end < 0) return null;
  const first = rows.value[start];
  if (!first) return null;
  if (first.kind === "dateGroup") {
    return offsets.value[start] < scrollTop.value ? first.label : null;
  }
  return first.group;
});

const activeDescendant = computed(() =>
  props.selectedOid ? rowId(`c:${props.selectedOid}`) : undefined,
);

function rowId(key: string): string {
  return `md-commit-row-${key.replace(/[^a-zA-Z0-9-]/g, "-")}`;
}

// --- Scrolling, anchoring, resizing ---------------------------------------

let scrollScheduled = false;
interface Anchor {
  key: string;
  delta: number;
}
let anchor: Anchor | null = null;
let observers: ResizeObserver | null = null;

function onScroll(): void {
  if (scrollScheduled) return;
  scrollScheduled = true;
  requestAnimationFrame(() => {
    scrollScheduled = false;
    const element = scroller.value;
    if (!element) return;
    scrollTop.value = element.scrollTop;
    captureAnchor();
  });
}

/** Remember which row is pinned at the top of the viewport, and where. */
function captureAnchor(): void {
  const element = scroller.value;
  if (!element) return;
  const list = rows.value;
  if (list.length === 0) {
    anchor = null;
    return;
  }
  const index = findRowAt(offsets.value, element.scrollTop);
  const row = list[index];
  if (!row || row.kind === "skeleton") {
    anchor = null;
    return;
  }
  anchor = { key: row.key, delta: offsets.value[index] - element.scrollTop };
}

/** Put the anchored row back where it was (paging, density, resize). */
function restoreAnchor(force = false): void {
  const element = scroller.value;
  const current = anchor;
  if (!element || !current) return;
  const index = keyIndex.value.get(current.key);
  if (index == null) return;
  const target = offsets.value[index] - current.delta;
  const max = Math.max(0, element.scrollHeight - element.clientHeight);
  const clamped = Math.min(Math.max(0, target), max);
  if (force || Math.abs(element.scrollTop - clamped) > 0.5) {
    element.scrollTop = clamped;
    scrollTop.value = clamped;
  }
}

watch(rows, () => {
  void nextTick(() => restoreAnchor());
});

function measure(): void {
  const element = scroller.value;
  if (!element) return;
  const width = element.clientWidth;
  const height = element.clientHeight;
  if (width === viewport.value.width && height === viewport.value.height) return;
  captureAnchor();
  viewport.value = { width, height };
  void nextTick(() => restoreAnchor());
}

onMounted(() => {
  measure();
  const element = scroller.value;
  if (element && typeof ResizeObserver !== "undefined") {
    observers = new ResizeObserver(() => measure());
    observers.observe(element);
  }
});

onBeforeUnmount(() => {
  observers?.disconnect();
  observers = null;
});

// --- Selection, keyboard ---------------------------------------------------

function commitNeighbor(from: number, delta: 1 | -1): number {
  const list = rows.value;
  let i = from;
  for (;;) {
    i += delta;
    if (i < 0 || i >= list.length) return -1;
    if (list[i].commitIndex != null) return i;
  }
}

function pageTarget(from: number, delta: 1 | -1): number {
  const step = Math.max(1, Math.floor(viewport.value.height / rowHeight.value) - 1);
  let i = from < 0 ? (delta > 0 ? -1 : rows.value.length) : from;
  for (let n = 0; n < step; n += 1) {
    const next = commitNeighbor(i, delta);
    if (next < 0) break;
    i = next;
  }
  return i;
}

function onKeydown(event: KeyboardEvent): void {
  const list = rows.value;
  if (list.length === 0) return;
  const current =
    props.selectedOid != null ? (keyIndex.value.get(`c:${props.selectedOid}`) ?? -1) : -1;
  let next = -1;
  switch (event.key) {
    case "ArrowDown":
      next = commitNeighbor(current, 1);
      break;
    case "ArrowUp":
      next = commitNeighbor(current < 0 ? list.length : current, -1);
      break;
    case "PageDown":
      next = pageTarget(current, 1);
      break;
    case "PageUp":
      next = pageTarget(current, -1);
      break;
    case "Home":
      next = commitNeighbor(-1, 1);
      break;
    case "End":
      next = commitNeighbor(list.length, -1);
      break;
    case "Enter": {
      // Open the selected commit (the same intent as a double-click).
      const selected = current >= 0 ? list[current] : null;
      const commit = selected?.commit;
      if (!commit) return;
      event.preventDefault();
      emit("activate", commit);
      return;
    }
    default:
      return;
  }
  if (next < 0) return;
  const commit = list[next].commit;
  if (!commit) return;
  event.preventDefault();
  emit("select", commit);
  scrollIndexIntoView(next, "nearest");
}

function scrollIndexIntoView(index: number, align: "nearest" | "start" | "center"): void {
  const element = scroller.value;
  const row = rows.value[index];
  if (!element || !row) return;
  const top = offsets.value[index];
  const bottom = top + row.height;
  const viewTop = element.scrollTop;
  const viewBottom = viewTop + element.clientHeight;
  let target = viewTop;
  if (align === "start") target = top;
  else if (align === "center") target = top - (element.clientHeight - row.height) / 2;
  else if (top < viewTop) target = top;
  else if (bottom > viewBottom) target = bottom - element.clientHeight;
  const max = Math.max(0, element.scrollHeight - element.clientHeight);
  target = Math.min(Math.max(0, target), max);
  if (Math.abs(element.scrollTop - target) > 0.5) {
    element.scrollTop = target;
    scrollTop.value = target;
    captureAnchor();
  }
}

watch(
  () => props.selectedOid,
  (oid) => {
    if (oid == null) return;
    const index = keyIndex.value.get(`c:${oid}`);
    if (index != null) scrollIndexIntoView(index, "nearest");
  },
);

function scrollToCommit(oid: string): boolean {
  const index = keyIndex.value.get(`c:${oid}`);
  if (index == null) return false;
  scrollIndexIntoView(index, "center");
  return true;
}

// --- Incremental paging ----------------------------------------------------

let requestedAtLength = -1;
let reachedEnd = false;

watch(
  [range, () => props.hasMore, () => props.loadingMore, () => rows.value.length],
  () => {
    const list = rows.value;
    const { end } = range.value;
    if (end < 0 || list.length === 0) return;
    const lastVisible = Math.min(
      list.length - 1,
      findRowAt(offsets.value, scrollTop.value + Math.max(0, viewport.value.height - 1)),
    );

    if (
      props.hasMore &&
      !props.loadingMore &&
      lastVisible >= list.length - 1 - props.pageThreshold &&
      list.length !== requestedAtLength
    ) {
      requestedAtLength = list.length;
      emit("need-more");
    }

    if (!props.hasMore && lastVisible >= list.length - 1) {
      if (!reachedEnd) {
        reachedEnd = true;
        emit("reach-end");
      }
    } else if (lastVisible < list.length - 2) {
      reachedEnd = false;
    }
  },
);

// --- Row events ------------------------------------------------------------

function onRowHover(key: string, commit: CommitInfo | null): void {
  if (commit) {
    hoveredKey.value = key;
  } else if (hoveredKey.value === key) {
    hoveredKey.value = null;
  }
  emit("hover", commit);
}

function isSelected(row: CommitDisplayRow): boolean {
  return props.selectedOid != null && row.commit != null && row.commit.oid === props.selectedOid;
}

defineExpose({ scrollToCommit });
</script>

<template>
  <div
    ref="scroller"
    class="virtual-commit-list"
    role="listbox"
    :aria-label="commitListStrings.listLabel"
    :aria-activedescendant="activeDescendant"
    tabindex="0"
    @scroll.passive="onScroll"
    @keydown="onKeydown"
  >
    <div class="list-spacer" :style="{ height: `${totalHeight}px` }">
      <div v-if="stickyLabel" class="sticky-group" role="presentation" aria-hidden="true">
        {{ stickyLabel }}
      </div>
      <CommitRow
        v-for="item in windowRows"
        :key="item.key"
        :row="item.row"
        :row-id="rowId(item.key)"
        :top="item.top"
        :height="item.height"
        :width="viewport.width"
        :density="density"
        :show-graph="showGraph"
        :selected="isSelected(item.row)"
        :hovered="hoveredKey === item.key"
        :theme="theme"
        :metrics="metrics"
        :graph="item.graph"
        @select="emit('select', $event)"
        @activate="emit('activate', $event)"
        @hover="onRowHover(item.key, $event)"
        @ref-click="emit('ref-click', $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.virtual-commit-list {
  position: relative;
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  background: var(--md-bg-surface);
  outline: none;
}

.virtual-commit-list:focus-visible {
  outline: 2px solid var(--md-accent);
  outline-offset: -2px;
}

.list-spacer {
  position: relative;
  width: 100%;
}

.sticky-group {
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  height: var(--md-row-compact);
  padding: 0 var(--md-space-3);
  background: var(--md-bg-header);
  border-bottom: 1px solid var(--md-border);
  color: var(--md-fg-muted);
  font-size: var(--md-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
</style>
