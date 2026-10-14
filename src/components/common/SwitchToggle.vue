<script setup lang="ts">
import Icon from "./Icon.vue";

/**
 * `<SwitchToggle>` — the bool control used by settings rows, dialogue options
 * and the commit composer. Presentational: emits the next value.
 */
withDefaults(
  defineProps<{
    modelValue: boolean;
    label?: string;
    disabled?: boolean;
    /** Screen-reader label when no visible label is rendered next to it. */
    ariaLabel?: string;
  }>(),
  { disabled: false },
);

const emit = defineEmits<{ "update:modelValue": [value: boolean] }>();
</script>

<template>
  <label class="switch-row" :class="{ disabled }">
    <button
      type="button"
      class="switch"
      role="switch"
      :aria-checked="modelValue"
      :aria-label="ariaLabel ?? label"
      :disabled="disabled"
      @click="emit('update:modelValue', !modelValue)"
    >
      <span class="knob" />
    </button>
    <span v-if="label" class="switch-label">{{ label }}</span>
    <span v-if="disabled" class="switch-lock">
      <Icon name="info.circle" :size="12" />
    </span>
  </label>
</template>

<style scoped>
.switch-row {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-2);
  cursor: pointer;
}

.switch-row.disabled {
  cursor: default;
  color: var(--md-fg-disabled);
}

.switch {
  width: 30px;
  height: 18px;
  border-radius: 9px;
  border: 1px solid var(--md-border-strong);
  background: var(--md-bg-sunken);
  padding: 0;
  position: relative;
  flex: none;
  transition: background var(--md-motion-fast) ease;
}

.switch[aria-checked="true"] {
  background: var(--md-accent);
  border-color: var(--md-accent);
}

.switch:disabled {
  opacity: 0.6;
}

.knob {
  position: absolute;
  top: 1px;
  left: 1px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--md-bg-surface);
  box-shadow: var(--md-shadow-1);
  transition: transform var(--md-motion-fast) ease;
}

.switch[aria-checked="true"] .knob {
  transform: translateX(12px);
}

.switch-label {
  font-size: var(--md-text-sm);
}

.switch-lock {
  color: var(--md-fg-subtle);
}
</style>
