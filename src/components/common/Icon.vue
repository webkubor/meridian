<script setup lang="ts">
import { computed } from "vue";

import { iconGlyph } from "./icon-set";

/**
 * `<Icon>` — the single icon renderer (docs/04 §3). `name` accepts either a
 * role (`nav.history`) or a symbol name (`clock`); unknown names fall back to
 * the registry's fallback glyph, so a role can never render nothing.
 */
const props = withDefaults(
  defineProps<{
    name: string;
    size?: number;
    strokeWidth?: number;
    /** Set when the icon carries meaning on its own; otherwise it is hidden. */
    label?: string;
  }>(),
  { size: 16, strokeWidth: 1.5 },
);

const segments = computed(() => iconGlyph(props.name));
</script>

<template>
  <svg
    class="md-icon"
    :width="size"
    :height="size"
    viewBox="0 0 16 16"
    fill="none"
    :aria-hidden="label ? undefined : 'true'"
    :role="label ? 'img' : undefined"
    :aria-label="label"
  >
    <path
      v-for="(segment, index) in segments"
      :key="index"
      :d="segment.d"
      :fill="segment.filled ? 'currentColor' : 'none'"
      :stroke="segment.filled ? 'none' : 'currentColor'"
      :stroke-width="strokeWidth"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
</template>

<style scoped>
.md-icon {
  display: block;
  flex: none;
}
</style>
