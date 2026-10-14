<script setup lang="ts">
import { ref } from "vue";

import type { FilterSectionModel } from "../contracts";
import Icon from "./Icon.vue";
import Popover from "./Popover.vue";

/**
 * `<FilterMenu>` — the per-pane view-settings menu (docs/04 §4: "sort menu,
 * filter menu, density toggle, view settings"). Purely presentational: it
 * renders whatever sections the pane passes and reports the toggled option id;
 * exclusivity of `radio` sections is the pane's rule.
 */
const props = withDefaults(
  defineProps<{
    label: string;
    icon?: string;
    sections: FilterSectionModel[];
    align?: "start" | "end";
    /** Renders as an icon-only button (list headers use this). */
    compact?: boolean;
  }>(),
  { align: "end", compact: true, icon: "line.3.horizontal.decrease" },
);

const emit = defineEmits<{
  toggle: [optionId: string];
  reset: [];
}>();

const open = ref(false);

function toggle(optionId: string): void {
  emit("toggle", optionId);
}

function reset(): void {
  emit("reset");
}
</script>

<template>
  <Popover :open="open" :align="align" :label="label" @close="open = false">
    <template #trigger>
      <button
        type="button"
        class="filter-trigger"
        :class="{ compact }"
        :title="label"
        :aria-expanded="open"
        aria-haspopup="menu"
        @click="open = !open"
      >
        <Icon :name="icon" :size="13" />
        <span v-if="!compact">{{ label }}</span>
      </button>
    </template>

    <div class="filter-panel">
      <div v-for="section in sections" :key="section.id" class="filter-section" role="group">
        <p class="filter-section-label">{{ section.label }}</p>
        <button
          v-for="option in section.options"
          :key="option.id"
          type="button"
          class="filter-option"
          role="menuitemcheckbox"
          :aria-checked="option.checked"
          @click="toggle(option.id)"
        >
          <span class="filter-tick" :class="{ checked: option.checked, radio: section.kind === 'radio' }">
            <Icon v-if="option.checked" name="checkmark" :size="11" :stroke-width="2" />
          </span>
          <span class="filter-option-label">{{ option.label }}</span>
          <span v-if="option.hint" class="filter-option-hint">{{ option.hint }}</span>
        </button>
      </div>
      <button type="button" class="filter-reset" role="menuitem" @click="reset">
        Reset view settings
      </button>
    </div>
  </Popover>
</template>

<style scoped>
.filter-trigger {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-1);
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  border-radius: var(--md-radius-sm);
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
}

.filter-trigger:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.filter-panel {
  min-width: 200px;
  max-height: 340px;
  overflow-y: auto;
}

.filter-section + .filter-section {
  border-top: 1px solid var(--md-border);
  margin-top: var(--md-space-1);
  padding-top: var(--md-space-1);
}

.filter-section-label {
  margin: 0;
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--md-fg-subtle);
}

.filter-option {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  border-radius: var(--md-radius-sm);
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
}

.filter-option:hover {
  background: var(--md-bg-hover);
}

.filter-tick {
  width: 14px;
  height: 14px;
  border-radius: 3px;
  border: 1px solid var(--md-border-strong);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--md-accent-contrast);
  flex: none;
}

.filter-tick.checked {
  background: var(--md-accent);
  border-color: var(--md-accent);
}

.filter-tick.radio {
  border-radius: 50%;
}

.filter-option-label {
  flex: 1;
  min-width: 0;
}

.filter-option-hint {
  color: var(--md-fg-subtle);
  font-size: var(--md-text-xs);
}

.filter-reset {
  display: block;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  border-top: 1px solid var(--md-border);
  margin-top: var(--md-space-1);
  padding: var(--md-space-1) var(--md-space-2);
  color: var(--md-accent);
  font-size: var(--md-text-sm);
  border-radius: 0 0 var(--md-radius-sm) var(--md-radius-sm);
}

.filter-reset:hover {
  background: var(--md-bg-hover);
}
</style>
