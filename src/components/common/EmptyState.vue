<script setup lang="ts">
import Icon from "./Icon.vue";

/**
 * `<EmptyState>` — icon + title + caption + optional actions (docs/04 §3).
 * Used by every list for its `empty` arm and by the detail panes for their
 * "nothing selected" states.
 */
withDefaults(
  defineProps<{
    /** An `empty.*` role from the icon registry. */
    icon: string;
    title: string;
    caption?: string;
    actionLabel?: string;
    secondaryActionLabel?: string;
    /** Compact variant for panes that are short on height. */
    compact?: boolean;
  }>(),
  { compact: false },
);

defineEmits<{
  action: [];
  secondaryAction: [];
}>();
</script>

<template>
  <div class="empty-state" :class="{ compact }">
    <span class="empty-mark">
      <Icon :name="icon" :size="compact ? 22 : 30" :stroke-width="1.2" />
    </span>
    <p class="empty-title">{{ title }}</p>
    <p v-if="caption" class="empty-caption">{{ caption }}</p>
    <div v-if="actionLabel || secondaryActionLabel" class="empty-actions">
      <button
        v-if="actionLabel"
        type="button"
        class="empty-primary"
        @click="$emit('action')"
      >
        {{ actionLabel }}
      </button>
      <button
        v-if="secondaryActionLabel"
        type="button"
        class="empty-secondary"
        @click="$emit('secondaryAction')"
      >
        {{ secondaryActionLabel }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--md-space-1);
  padding: var(--md-space-6) var(--md-space-4);
  text-align: center;
  height: 100%;
  min-height: 0;
}

.empty-state.compact {
  padding: var(--md-space-4) var(--md-space-3);
}

.empty-mark {
  color: var(--md-fg-subtle);
  margin-bottom: var(--md-space-1);
}

.empty-title {
  margin: 0;
  font-size: var(--md-text-md);
  font-weight: 600;
}

.empty-caption {
  margin: 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg-muted);
  max-width: 380px;
}

.empty-actions {
  display: flex;
  gap: var(--md-space-2);
  margin-top: var(--md-space-2);
}

.empty-primary {
  background: var(--md-accent);
  border-color: var(--md-accent);
  color: var(--md-accent-contrast);
}

.empty-primary:hover:not(:disabled) {
  background: var(--md-accent-strong);
}
</style>
