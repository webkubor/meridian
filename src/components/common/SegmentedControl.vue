<script setup lang="ts">
/**
 * `<SegmentedControl>` — the small exclusive picker used for view modes
 * (unified/split, density) and dialogue option sets. Radios under the hood, so
 * arrow keys and screen readers behave.
 */
withDefaults(
  defineProps<{
    modelValue: string;
    options: { id: string; label: string }[];
    label: string;
    disabled?: boolean;
  }>(),
  { disabled: false },
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();
</script>

<template>
  <div class="segmented" role="radiogroup" :aria-label="label">
    <button
      v-for="option in options"
      :key="option.id"
      type="button"
      role="radio"
      :aria-checked="option.id === modelValue"
      class="segment"
      :class="{ active: option.id === modelValue }"
      :disabled="disabled"
      @click="emit('update:modelValue', option.id)"
    >
      {{ option.label }}
    </button>
  </div>
</template>

<style scoped>
.segmented {
  display: inline-flex;
  padding: 2px;
  gap: 2px;
  border: 1px solid var(--md-border-strong);
  border-radius: var(--md-radius-md);
  background: var(--md-bg-sunken);
}

.segment {
  border: none;
  background: transparent;
  border-radius: calc(var(--md-radius-md) - 2px);
  padding: 1px var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}

.segment:hover:not(:disabled) {
  background: transparent;
  color: var(--md-fg);
}

.segment.active {
  background: var(--md-bg-surface);
  color: var(--md-fg);
  box-shadow: var(--md-shadow-1);
  font-weight: 600;
}

.segment:disabled {
  color: var(--md-fg-disabled);
}
</style>
