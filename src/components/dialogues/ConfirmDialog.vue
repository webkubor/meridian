<script setup lang="ts">
import DialogueShell from "./DialogueShell.vue";
import ProgressTrack from "../common/ProgressTrack.vue";

/**
 * `<ConfirmDialog>` — the confirmation pattern (docs/04 §8.2). The message
 * names the consequence; `items` lists exactly what is affected. For
 * destructive verbs the primary button carries the danger tone and initial
 * focus stays on Cancel, so an Enter press never destroys work.
 */
withDefaults(
  defineProps<{
    title: string;
    message: string;
    items?: string[];
    confirmLabel: string;
    cancelLabel?: string;
    danger?: boolean;
    busy?: boolean;
    progress?: { done: number; total: number; phase: string | null } | null;
  }>(),
  {
    cancelLabel: "Cancel",
    danger: false,
    busy: false,
    progress: null,
  },
);

const emit = defineEmits<{ confirm: []; cancel: [] }>();
</script>

<template>
  <DialogueShell
    :title="title"
    :danger="danger"
    :size="items && items.length > 0 ? 'm' : 's'"
    :busy="busy"
    @close="emit('cancel')"
  >
    <p class="confirm-message">{{ message }}</p>
    <ul v-if="items && items.length > 0" class="confirm-items">
      <li v-for="(item, index) in items" :key="index">{{ item }}</li>
    </ul>

    <div v-if="progress" class="confirm-progress">
      <ProgressTrack
        state="determinate"
        :done="progress.done"
        :total="progress.total"
        :phase="progress.phase"
      />
    </div>

    <template #footer>
      <button type="button" class="btn" :data-autofocus="danger ? '' : undefined" @click="emit('cancel')">
        {{ cancelLabel }}
      </button>
      <button
        type="button"
        class="btn primary"
        :class="{ danger }"
        :data-autofocus="danger ? undefined : ''"
        :disabled="busy"
        @click="emit('confirm')"
      >
        {{ confirmLabel }}
      </button>
    </template>
  </DialogueShell>
</template>

<style scoped>
.confirm-message {
  margin: 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg);
}

.confirm-items {
  margin: var(--md-space-2) 0 0;
  padding: var(--md-space-2) var(--md-space-2) var(--md-space-2) var(--md-space-5);
  list-style: disc;
  background: var(--md-bg-sunken);
  border-radius: var(--md-radius-md);
  font-size: var(--md-text-sm);
  color: var(--md-fg-muted);
}

.confirm-items li {
  overflow-wrap: anywhere;
}

.confirm-progress {
  margin-top: var(--md-space-3);
}
</style>
