<script setup lang="ts">
import { onBeforeUnmount, watch, ref } from "vue";

/**
 * `<Popover>` — the only floating-panel primitive (docs/04 §2.4: level-1
 * elevation). Anchors to its trigger, closes on outside pointer-down and on
 * Escape. Keyboard users reach the trigger normally; the panel is a `menu`.
 */
const props = withDefaults(
  defineProps<{
    open: boolean;
    align?: "start" | "end";
    /** Accessible name for the panel, when it is a menu. */
    label?: string;
  }>(),
  { align: "start" },
);

const emit = defineEmits<{ close: [] }>();

const root = ref<HTMLElement | null>(null);

function onPointerDown(event: PointerEvent): void {
  const target = event.target as Node | null;
  if (root.value && target && !root.value.contains(target)) emit("close");
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape") {
    event.stopPropagation();
    emit("close");
  }
}

function bind(active: boolean): void {
  if (active) {
    document.addEventListener("pointerdown", onPointerDown, true);
    document.addEventListener("keydown", onKeydown);
  } else {
    document.removeEventListener("pointerdown", onPointerDown, true);
    document.removeEventListener("keydown", onKeydown);
  }
}

watch(() => props.open, bind);
onBeforeUnmount(() => bind(false));
</script>

<template>
  <div ref="root" class="popover-anchor">
    <slot name="trigger" />
    <div
      v-if="open"
      class="popover-panel"
      :class="`align-${align}`"
      role="menu"
      :aria-label="label"
    >
      <slot />
    </div>
  </div>
</template>

<style scoped>
.popover-anchor {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.popover-panel {
  position: absolute;
  top: calc(100% + 4px);
  min-width: 180px;
  max-width: 340px;
  padding: var(--md-space-1);
  background: var(--md-bg-surface);
  border-radius: var(--md-radius-md);
  box-shadow: var(--md-shadow-2);
  z-index: var(--md-z-popover);
}

.popover-panel.align-start {
  left: 0;
}

.popover-panel.align-end {
  right: 0;
}
</style>
