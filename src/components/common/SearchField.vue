<script setup lang="ts">
import Icon from "./Icon.vue";

/**
 * `<SearchField>` — the per-pane search affordance (docs/04 §4 "search
 * (in-pane)"). Presentational: the value lives with the pane's filter model.
 */
withDefaults(
  defineProps<{
    modelValue: string;
    placeholder?: string;
    label: string;
  }>(),
  { placeholder: "Search" },
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

function onInput(event: Event): void {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}

function clear(): void {
  emit("update:modelValue", "");
}
</script>

<template>
  <label class="search-field">
    <Icon name="magnifyingglass" :size="13" />
    <input
      type="search"
      :value="modelValue"
      :placeholder="placeholder"
      :aria-label="label"
      spellcheck="false"
      @input="onInput"
      @keydown.escape="clear"
    />
    <button v-if="modelValue" type="button" class="clear" aria-label="Clear search" @click="clear">
      <Icon name="xmark.circle.fill" :size="12" />
    </button>
  </label>
</template>

<style scoped>
.search-field {
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  padding: 0 var(--md-space-2);
  height: 24px;
  border: 1px solid var(--md-border-strong);
  border-radius: var(--md-radius-md);
  background: var(--md-bg-surface);
  color: var(--md-fg-subtle);
  min-width: 120px;
  flex: 1;
}

.search-field:focus-within {
  border-color: var(--md-accent);
}

.search-field input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  padding: 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg);
}

.search-field input:focus-visible {
  outline: none;
}

.search-field input::-webkit-search-cancel-button {
  display: none;
}

.clear {
  border: none;
  background: transparent;
  padding: 0;
  color: var(--md-fg-subtle);
  display: inline-flex;
}

.clear:hover {
  background: transparent;
  color: var(--md-fg-muted);
}
</style>
