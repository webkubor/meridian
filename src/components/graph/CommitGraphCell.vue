<script setup lang="ts">
/**
 * One row's graph cell: a canvas spanning the row, on which the lane strokes,
 * the commit bubble and the ref badge chips are painted (evidence:
 * `GTHistoryCommitListGraphView` is a view inside every commit cell).
 *
 * The cell is a pure view over props; the list above it owns the model (lane
 * assignment index, row offsets, badge strip). Repaints are coalesced to one
 * animation frame and are triggered by the props the paint actually reads, so
 * scrolling a recycled row that keeps its content costs nothing, while
 * selection/hover/theme changes repaint only the rows they touch.
 */
import { onBeforeUnmount, onMounted, ref, watchEffect } from "vue";

import type { BadgeStrip } from "./badges";
import type { GraphMetrics } from "./geometry";
import { paintRow, type PaintInput } from "./renderer";
import type { GraphTheme } from "./theme";
import type { GraphLaneSegment } from "./types";

const props = defineProps<{
  /** Row width in CSS pixels (the canvas paints the whole row: graph + badges). */
  width: number;
  /** Row height in CSS pixels. */
  height: number;
  /** Content-space y of the row's top edge. */
  rowTop: number;
  /** Layout row index of this row's commit (`null` for rows without one). */
  commitRow: number | null;
  lane: number | null;
  segments: readonly GraphLaneSegment[];
  centers: Float64Array;
  strip: BadgeStrip | null;
  stripOriginX: number;
  hoveredChip: number;
  continuationTop: readonly number[];
  continuationBottom: readonly number[];
  metrics: GraphMetrics;
  theme: GraphTheme;
  selected: boolean;
  hovered: boolean;
  isHead: boolean;
}>();

const canvas = ref<HTMLCanvasElement | null>(null);
let pending: PaintInput | null = null;
let frame = 0;

watchEffect(() => {
  pending = {
    width: props.width,
    height: props.height,
    rowTop: props.rowTop,
    rowHeight: props.height,
    commitRow: props.commitRow,
    lane: props.lane,
    segments: props.segments,
    centers: props.centers,
    strip: props.strip,
    stripOriginX: props.stripOriginX,
    hoveredChip: props.hoveredChip,
    continuation: { top: props.continuationTop, bottom: props.continuationBottom },
    metrics: props.metrics,
    theme: props.theme,
    selected: props.selected,
    hovered: props.hovered,
    isHead: props.isHead,
  };
  schedule();
});

function schedule(): void {
  if (frame !== 0) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    paint();
  });
}

function paint(): void {
  const element = canvas.value;
  const input = pending;
  if (!element || !input || input.width <= 0 || input.height <= 0) return;

  const dpr = Math.max(1, Math.min(3, window.devicePixelRatio || 1));
  const pixelWidth = Math.round(input.width * dpr);
  const pixelHeight = Math.round(input.height * dpr);
  if (element.width !== pixelWidth || element.height !== pixelHeight) {
    element.width = pixelWidth;
    element.height = pixelHeight;
  }

  const ctx = element.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  paintRow(ctx, input);
}

onMounted(schedule);
onBeforeUnmount(() => {
  if (frame !== 0) cancelAnimationFrame(frame);
  frame = 0;
  pending = null;
});
</script>

<template>
  <canvas
    ref="canvas"
    class="graph-cell"
    aria-hidden="true"
    :style="{ width: `${width}px`, height: `${height}px` }"
  />
</template>

<style scoped>
.graph-cell {
  position: absolute;
  top: 0;
  left: 0;
  display: block;
  pointer-events: none;
}
</style>
