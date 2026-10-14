<script setup lang="ts">
/**
 * One display row of the commit list: a date-group header, a loading skeleton,
 * or a commit (normal / merge / fork point).
 *
 * The commit row composes the graph cell canvas (graph module) with its text
 * columns and owns the ref-badge hit testing: badge chips are painted on the
 * canvas, but pointing at them must feel like pointing at a control, so the row
 * maps mouse position over the measured strip (utils from graph/badges) to a
 * chip and reports it back to the canvas as `hoveredChip`.
 */
import { computed, ref, watch } from "vue";

import { CommitGraphCell, hitTestStrip, type GraphDensity, type GraphMetrics, type GraphTheme } from "../graph";
import { formatExact, formatWhen, type CommitDisplayRow, type RowGraph } from "./rows";
import { commitListStrings } from "./strings";

const props = defineProps<{
  row: CommitDisplayRow;
  /** DOM id used by the list's `aria-activedescendant`. */
  rowId: string;
  /** Content-space top of the row. */
  top: number;
  height: number;
  /** Row width in CSS pixels (for the canvas). */
  width: number;
  density: GraphDensity;
  showGraph: boolean;
  selected: boolean;
  hovered: boolean;
  theme: GraphTheme;
  metrics: GraphMetrics;
  graph: RowGraph | null;
}>();

const emit = defineEmits<{
  select: [commit: NonNullable<CommitDisplayRow["commit"]>];
  activate: [commit: NonNullable<CommitDisplayRow["commit"]>];
  hover: [commit: NonNullable<CommitDisplayRow["commit"]> | null];
  "ref-click": [payload: { commit: NonNullable<CommitDisplayRow["commit"]>; ref: string; label: string }];
}>();

const element = ref<HTMLElement | null>(null);
const hoveredChip = ref(-1);

watch(
  () => props.graph?.strip,
  () => {
    hoveredChip.value = -1;
  },
);

const commit = computed(() => props.row.commit);
const when = computed(() => (commit.value ? formatWhen(commit.value.committer.time, Date.now() / 1000) : ""));

const chip = computed(() => {
  const chips = props.graph?.strip?.chips;
  if (!chips || hoveredChip.value < 0 || hoveredChip.value >= chips.length) return null;
  return chips[hoveredChip.value];
});

const rowTitle = computed(() => {
  if (!commit.value) return "";
  return `${commit.value.summary}\n${commit.value.author.name} · ${formatExact(commit.value.committer.time)}`;
});

const title = computed(() => chip.value?.badge.ref ?? chip.value?.badge.label ?? rowTitle.value);

const ariaLabel = computed(() => {
  if (!commit.value) return undefined;
  return commitListStrings.rowLabel(
    commit.value.summary,
    commit.value.author.name,
    formatExact(commit.value.committer.time),
  );
});

const style = computed(() => ({
  top: `${props.top}px`,
  height: `${props.height}px`,
}));

function onMove(event: MouseEvent): void {
  const strip = props.graph?.strip;
  const host = element.value;
  if (!strip || !host) {
    setChip(-1);
    return;
  }
  const rect = host.getBoundingClientRect();
  const x = event.clientX - rect.left - props.graph!.stripOriginX;
  setChip(hitTestStrip(strip, x));
}

function setChip(index: number): void {
  if (hoveredChip.value !== index) hoveredChip.value = index;
}

function onClick(): void {
  if (!commit.value) return;
  const active = chip.value;
  if (active && active.badge.id !== "overflow") {
    emit("ref-click", {
      commit: commit.value,
      ref: active.badge.ref ?? active.badge.label,
      label: active.badge.label,
    });
  }
  emit("select", commit.value);
}

function onActivate(): void {
  if (commit.value) emit("activate", commit.value);
}

function onEnter(): void {
  if (commit.value) emit("hover", commit.value);
}

function onLeave(): void {
  setChip(-1);
  emit("hover", null);
}
</script>

<template>
  <div
    v-if="row.kind === 'dateGroup'"
    class="group-row"
    role="presentation"
    aria-hidden="true"
    :style="style"
  >
    <span class="group-label">{{ row.label }}</span>
  </div>

  <div
    v-else-if="row.kind === 'skeleton'"
    class="skeleton-row"
    role="presentation"
    aria-hidden="true"
    :style="style"
    :aria-label="commitListStrings.skeletonAria"
  >
    <span class="skeleton-bar bar-graph" />
    <span class="skeleton-bar bar-text" />
    <span class="skeleton-bar bar-meta" />
  </div>

  <div
    v-else
    ref="element"
    class="commit-row"
    :class="[`kind-${row.kind}`, { selected, hovered, 'over-badge': hoveredChip >= 0 }]"
    role="option"
    :id="rowId"
    :style="style"
    :aria-selected="selected"
    :aria-label="ariaLabel"
    :title="title"
    @click="onClick"
    @dblclick="onActivate"
    @mouseenter="onEnter"
    @mouseleave="onLeave"
    @mousemove="onMove"
  >
    <CommitGraphCell
      v-if="showGraph && graph"
      :width="width"
      :height="height"
      :row-top="top"
      :commit-row="graph.layoutRow"
      :lane="graph.lane"
      :segments="graph.segments"
      :centers="graph.centers"
      :strip="graph.strip"
      :strip-origin-x="graph.stripOriginX"
      :hovered-chip="hoveredChip"
      :continuation-top="graph.continuationTop"
      :continuation-bottom="graph.continuationBottom"
      :metrics="metrics"
      :theme="theme"
      :selected="selected"
      :hovered="hovered"
      :is-head="graph.isHead"
    />
    <div class="commit-text" :style="{ paddingLeft: `${graph ? graph.inset : 0}px` }">
      <span class="commit-summary">{{ commit?.summary }}</span>
      <span v-if="row.kind === 'forkPoint'" class="fork-chip">{{ commitListStrings.forkPointBadge }}</span>
      <span class="commit-spacer" />
      <span class="commit-author">{{ commit?.author.name }}</span>
      <span class="commit-when">{{ when }}</span>
    </div>
  </div>
</template>

<style scoped>
.group-row,
.skeleton-row,
.commit-row {
  position: absolute;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
}

.group-row {
  padding: 0 var(--md-space-3);
  background: var(--md-bg-header);
  border-bottom: 1px solid var(--md-border);
  color: var(--md-fg-muted);
  font-size: var(--md-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.commit-row {
  padding-right: var(--md-space-3);
  color: var(--md-fg);
  font-size: var(--md-text-md);
  line-height: var(--md-leading-tight);
  cursor: default;
  contain: layout style;
}

.commit-row.kind-forkPoint {
  background: var(--md-bg-sunken);
}

.commit-row.kind-merge .commit-summary {
  font-weight: 600;
}

.commit-row:hover,
.commit-row.kind-forkPoint:hover {
  background: var(--md-bg-hover);
}

.commit-row.selected,
.commit-row.selected:hover {
  background: var(--md-bg-selected);
}

.commit-row.over-badge {
  cursor: pointer;
}

.commit-text {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: var(--md-space-3);
  flex: 1 1 auto;
  min-width: 0;
  height: 100%;
}

.commit-summary {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.commit-spacer {
  flex: 0 0 0;
}

.fork-chip {
  flex: 0 0 auto;
  padding: 0 var(--md-space-2);
  border: 1px solid var(--md-border-strong);
  border-radius: var(--md-radius-sm);
  color: var(--md-fg-subtle);
  font-size: var(--md-text-xs);
}

.commit-author {
  flex: 0 1 auto;
  max-width: 28%;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--md-fg-muted);
  font-size: var(--md-text-sm);
}

.commit-when {
  flex: 0 0 auto;
  color: var(--md-fg-subtle);
  font-size: var(--md-text-sm);
  font-variant-numeric: tabular-nums;
}

.skeleton-row {
  gap: var(--md-space-3);
  padding: 0 var(--md-space-3);
}

.skeleton-bar {
  height: 8px;
  border-radius: var(--md-radius-sm);
  background: var(--md-bg-hover);
  animation: md-skeleton 1.4s ease-in-out infinite;
}

.bar-graph {
  width: 40px;
}

.bar-text {
  flex: 1 1 auto;
  max-width: 420px;
}

.bar-meta {
  width: 120px;
}

@keyframes md-skeleton {
  0%,
  100% {
    opacity: 0.55;
  }
  50% {
    opacity: 1;
  }
}
</style>
