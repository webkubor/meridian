<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";

/**
 * `<MasterDetail>` — the list + detail split used by working copy, history,
 * branches and compare (docs/04 §2.2). The divider drags to resize the detail
 * column; the width is a prop so the shell persists it. The separator is a
 * real keyboard control (arrow keys), not decoration (docs/04 §9).
 */
const props = withDefaults(
  defineProps<{
    detailWidth: number;
    min?: number;
    max?: number;
    /** Which side the detail column sits on. */
    side?: "end" | "start";
    detailVisible?: boolean;
  }>(),
  { min: 260, max: 760, side: "end", detailVisible: true },
);

const emit = defineEmits<{ "update:detailWidth": [value: number] }>();

const root = ref<HTMLElement | null>(null);
const dragging = ref(false);

function clamp(value: number): number {
  return Math.min(props.max, Math.max(props.min, value));
}

function applyFromPointer(clientX: number): void {
  const element = root.value;
  if (!element) return;
  const rect = element.getBoundingClientRect();
  const raw = props.side === "end" ? rect.right - clientX : clientX - rect.left;
  emit("update:detailWidth", clamp(raw));
}

function onPointerMove(event: PointerEvent): void {
  if (!dragging.value) return;
  applyFromPointer(event.clientX);
}

function onPointerUp(): void {
  if (!dragging.value) return;
  dragging.value = false;
  document.removeEventListener("pointermove", onPointerMove);
  document.removeEventListener("pointerup", onPointerUp);
  document.body.style.cursor = "";
  document.body.style.userSelect = "";
}

function startDrag(event: PointerEvent): void {
  dragging.value = true;
  document.addEventListener("pointermove", onPointerMove);
  document.addEventListener("pointerup", onPointerUp);
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  event.preventDefault();
}

function onKeydown(event: KeyboardEvent): void {
  const step = event.shiftKey ? 48 : 16;
  if (event.key === "ArrowLeft") {
    emit("update:detailWidth", clamp(props.detailWidth + (props.side === "end" ? step : -step)));
    event.preventDefault();
  } else if (event.key === "ArrowRight") {
    emit("update:detailWidth", clamp(props.detailWidth - (props.side === "end" ? step : -step)));
    event.preventDefault();
  } else if (event.key === "Home") {
    emit("update:detailWidth", props.min);
    event.preventDefault();
  } else if (event.key === "End") {
    emit("update:detailWidth", props.max);
    event.preventDefault();
  }
}

onBeforeUnmount(() => {
  onPointerUp();
});
</script>

<template>
  <div ref="root" class="master-detail" :class="`side-${side}`">
    <div class="master"><slot name="master" /></div>
    <div
      v-if="detailVisible"
      class="divider"
      role="separator"
      aria-orientation="vertical"
      :aria-label="$t('screens.masterDetail.resizeLabel')"
      :aria-valuenow="detailWidth"
      :aria-valuemin="min"
      :aria-valuemax="max"
      tabindex="0"
      :class="{ dragging }"
      @pointerdown="startDrag"
      @keydown="onKeydown"
    />
    <div v-if="detailVisible" class="detail" :style="{ width: `${detailWidth}px` }">
      <slot name="detail" />
    </div>
  </div>
</template>

<style scoped>
.master-detail {
  display: flex;
  height: 100%;
  min-height: 0;
  min-width: 0;
  flex: 1;
}

.master-detail.side-start {
  flex-direction: row-reverse;
}

.master {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.detail {
  flex: none;
  min-height: 0;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--md-border);
}

.side-start .detail {
  border-left: none;
  border-right: 1px solid var(--md-border);
}

.divider {
  flex: none;
  width: 5px;
  margin: 0 -2px;
  cursor: col-resize;
  background: transparent;
  position: relative;
  z-index: 1;
}

.divider::after {
  content: "";
  position: absolute;
  inset: 0 2px;
  background: transparent;
}

.divider:hover::after,
.divider.dragging::after {
  background: var(--md-accent);
}

.divider:focus-visible {
  outline: none;
}

.divider:focus-visible::after {
  background: var(--md-accent);
}
</style>
