<script setup lang="ts">
import { ref } from "vue";

import type { SplitActionOption } from "../contracts";
import Icon from "./Icon.vue";
import Popover from "./Popover.vue";

/**
 * `<SplitAction>` — the default-action + caret pattern (docs/04 §1.3, §8.1):
 * click runs the default verb, the caret opens the options panel, and a full
 * dialogue is what an option opens when it has one. Used on branch rows,
 * toolbar verbs, stash apply, tag publish, remote fetch.
 */
const props = withDefaults(
  defineProps<{
    label: string;
    icon?: string;
    disabled?: boolean;
    /** Tooltip shown when disabled — never a silent dead control. */
    reason?: string;
    busy?: boolean;
    danger?: boolean;
    options?: SplitActionOption[];
    compact?: boolean;
  }>(),
  { compact: false },
);

const emit = defineEmits<{
  run: [];
  choose: [id: string];
}>();

const open = ref(false);

function runPrimary(): void {
  if (props.disabled || props.busy) return;
  emit("run");
}

function choose(id: string): void {
  open.value = false;
  emit("choose", id);
}
</script>

<template>
  <Popover :open="open" align="end" :label="`${label} options`" @close="open = false">
    <template #trigger>
      <span class="split-action" :class="{ compact, danger }">
        <button
          type="button"
          class="split-primary"
          :disabled="disabled || busy"
          :title="disabled ? reason : label"
          :aria-busy="busy || undefined"
          @click="runPrimary"
        >
          <span v-if="busy" class="mini-spinner" aria-hidden="true" />
          <Icon v-else-if="icon" :name="icon" :size="compact ? 13 : 14" />
          <span class="split-label">{{ label }}</span>
        </button>
        <button
          v-if="options && options.length > 0"
          type="button"
          class="split-caret"
          :disabled="disabled"
          :title="reason"
          :aria-expanded="open"
          aria-haspopup="menu"
          @click="open = !open"
        >
          <Icon name="chevron.down" :size="11" />
        </button>
      </span>
    </template>

    <button
      v-for="option in options"
      :key="option.id"
      type="button"
      class="popover-item"
      role="menuitem"
      :class="{ danger: option.danger }"
      :disabled="option.disabled"
      :title="option.disabled ? option.reason : undefined"
      @click="choose(option.id)"
    >
      {{ option.label }}
    </button>
  </Popover>
</template>

<style scoped>
.split-action {
  display: inline-flex;
  align-items: stretch;
  border: 1px solid var(--md-border-strong);
  border-radius: var(--md-radius-md);
  background: var(--md-bg-surface);
  overflow: hidden;
}

.split-action.danger .split-primary {
  color: var(--md-danger);
}

.split-primary {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-1);
  border: none;
  border-radius: 0;
  background: transparent;
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
}

.split-caret {
  display: inline-flex;
  align-items: center;
  border: none;
  border-left: 1px solid var(--md-border);
  border-radius: 0;
  background: transparent;
  padding: 0 5px;
  color: var(--md-fg-muted);
}

.split-primary:hover:not(:disabled),
.split-caret:hover:not(:disabled) {
  background: var(--md-bg-hover);
}

.split-action:has(.split-primary:disabled) {
  opacity: 0.55;
}

.split-label {
  white-space: nowrap;
}

.compact .split-primary {
  padding: 2px var(--md-space-2);
}

.mini-spinner {
  width: 11px;
  height: 11px;
  border-radius: 50%;
  border: 2px solid var(--md-border-strong);
  border-top-color: var(--md-accent);
  animation: md-spin 0.8s linear infinite;
}

@keyframes md-spin {
  to {
    transform: rotate(360deg);
  }
}

.popover-item {
  display: block;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  border-radius: var(--md-radius-sm);
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
}

.popover-item:hover:not(:disabled) {
  background: var(--md-bg-hover);
}

.popover-item.danger {
  color: var(--md-danger);
}

.popover-item:disabled {
  color: var(--md-fg-disabled);
}
</style>
