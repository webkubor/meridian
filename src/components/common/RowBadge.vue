<script setup lang="ts">
import { computed } from "vue";

import type { Tone } from "../../bridges/bindings";
import Icon from "./Icon.vue";

/**
 * `<RowBadge>` — the badge vocabulary of docs/04 §3/§4.1: counts, ahead/behind,
 * state badges. A badge always carries an icon or a number, never colour alone
 * (docs/04 §9).
 */
const props = withDefaults(
  defineProps<{
    label?: string | number;
    icon?: string;
    tone?: Tone;
    /** Selected-row variant uses the selected badge colours. */
    selected?: boolean;
  }>(),
  { tone: "neutral" },
);

const text = computed(() => (props.label === undefined ? null : String(props.label)));
</script>

<template>
  <span class="row-badge" :class="[`tone-${tone}`, { selected }]">
    <Icon v-if="icon" :name="icon" :size="11" :stroke-width="1.6" />
    <span v-if="text" class="row-badge-text">{{ text }}</span>
  </span>
</template>

<style scoped>
.row-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 0 var(--md-space-1);
  min-width: 16px;
  height: 16px;
  justify-content: center;
  border-radius: var(--md-radius-sm);
  background: var(--md-badge-bg);
  color: var(--md-badge-fg);
  font-size: var(--md-text-xs);
  font-variant-numeric: tabular-nums;
  line-height: 1;
  flex: none;
}

.row-badge.selected {
  background: var(--md-badge-selected-bg);
  color: var(--md-badge-selected-fg);
}

.row-badge-text {
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 84px;
  white-space: nowrap;
}
</style>
