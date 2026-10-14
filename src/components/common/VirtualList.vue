<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

/**
 * `<VirtualList>` — windowed rows (docs/04 §6).
 *
 * The parent owns the data and renders one row per visible index through the
 * default scoped slot; the list owns the scroll container, the windowing and
 * the sticky-free geometry. Row heights may be uniform or per-index (the
 * history list interleaves date-group and commit rows).
 *
 * It reports `scroll` and `viewport` so sibling surfaces that must stay in
 * vertical lockstep (the commit-graph canvas) can draw the same window.
 */
const props = withDefaults(
  defineProps<{
    count: number;
    /** Uniform height, or a function of the row index. */
    rowHeight: number | ((index: number) => number);
    overscan?: number;
    /** Extra class on the scroll viewport. */
    viewportClass?: string;
  }>(),
  { overscan: 8 },
);

const emit = defineEmits<{
  scroll: [scrollTop: number];
  viewport: [height: number];
  /** Fired when the user reaches the end (drives "load more"). */
  endReached: [];
}>();

const viewport = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewportHeight = ref(0);

const offsets = computed<number[]>(() => {
  const height = props.rowHeight;
  if (typeof height === "number") {
    return Array.from({ length: props.count + 1 }, (_, index) => index * height);
  }
  const table: number[] = [0];
  for (let index = 0; index < props.count; index += 1) {
    table.push(table[index] + height(index));
  }
  return table;
});

/** Overscan in pixels; uniform lists scale it by the row height. */
const overscanPx = computed(() =>
  props.overscan * (typeof props.rowHeight === "number" ? props.rowHeight : 28),
);

const start = computed(() => {
  const table = offsets.value;
  const probe = scrollTop.value - overscanPx.value;
  let low = 0;
  let high = props.count;
  while (low < high) {
    const mid = (low + high + 1) >> 1;
    if (table[mid] <= probe) low = mid;
    else high = mid - 1;
  }
  return Math.max(0, low);
});

const end = computed(() => {
  const table = offsets.value;
  const probe = scrollTop.value + viewportHeight.value + overscanPx.value;
  let low = start.value;
  let high = props.count;
  while (low < high) {
    const mid = (low + high + 1) >> 1;
    if (table[mid] <= probe) low = mid;
    else high = mid - 1;
  }
  return Math.min(props.count, low);
});

const visible = computed(() => {
  const indexes: number[] = [];
  for (let index = start.value; index < end.value; index += 1) indexes.push(index);
  return indexes;
});

function rowHeightAt(index: number): number {
  const height = props.rowHeight;
  return typeof height === "number" ? height : height(index);
}

const topOffset = computed(() => offsets.value[start.value] ?? 0);
const totalHeight = computed(() => offsets.value[props.count] ?? 0);

function onScroll(): void {
  const element = viewport.value;
  if (!element) return;
  scrollTop.value = element.scrollTop;
  emit("scroll", element.scrollTop);
  if (element.scrollTop + element.clientHeight >= element.scrollHeight - 2) {
    emit("endReached");
  }
}

let observer: ResizeObserver | null = null;

function measure(): void {
  const element = viewport.value;
  if (!element) return;
  viewportHeight.value = element.clientHeight;
  emit("viewport", element.clientHeight);
}

onMounted(() => {
  measure();
  if (viewport.value && typeof ResizeObserver !== "undefined") {
    observer = new ResizeObserver(() => measure());
    observer.observe(viewport.value);
  }
});

onBeforeUnmount(() => {
  observer?.disconnect();
});

watch(
  () => props.count,
  () => {
    const element = viewport.value;
    if (!element) return;
    const max = Math.max(0, totalHeight.value - element.clientHeight);
    if (element.scrollTop > max) {
      element.scrollTop = max;
      scrollTop.value = max;
      emit("scroll", max);
    }
  },
);

defineExpose({ scrollToTop });
function scrollToTop(): void {
  if (viewport.value) viewport.value.scrollTop = 0;
}
</script>

<template>
  <div
    ref="viewport"
    class="virtual-list"
    :class="viewportClass"
    role="list"
    @scroll.passive="onScroll"
  >
    <div class="virtual-canvas" :style="{ height: `${totalHeight}px` }">
      <div class="virtual-window" :style="{ transform: `translateY(${topOffset}px)` }">
        <div
          v-for="index in visible"
          :key="index"
          class="virtual-row"
          :style="{ height: `${rowHeightAt(index)}px` }"
          role="listitem"
        >
          <slot :index="index" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.virtual-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  position: relative;
}

.virtual-canvas {
  position: relative;
  width: 100%;
}

.virtual-window {
  position: absolute;
  inset: 0 0 auto 0;
}

.virtual-row {
  overflow: hidden;
}
</style>
