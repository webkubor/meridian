<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import DialogueShell from "./DialogueShell.vue";
import ProgressTrack from "../common/ProgressTrack.vue";

/**
 * `<ConfirmDialog>` — the confirmation pattern (docs/04 §8.2). The message
 * names the consequence; `items` lists exactly what is affected. For
 * destructive verbs the primary button carries the danger tone and initial
 * focus stays on Cancel, so an Enter press never destroys work.
 */
const props = withDefaults(
  defineProps<{
    title: string;
    message: string;
    items?: string[];
    confirmLabel: string;
    cancelLabel?: string;
    /** Verbatim text the confirm button will transmit; read before it does. */
    preview?: string;
    previewLabel?: string;
    secondaryLabel?: string;
    danger?: boolean;
    busy?: boolean;
    progress?: { done: number; total: number; phase: string | null } | null;
  }>(),
  {
    danger: false,
    busy: false,
    progress: null,
  },
);

const emit = defineEmits<{ confirm: []; cancel: []; secondary: [] }>();

const { t } = useI18n();

/**
 * The default comes from the catalogue rather than `withDefaults`, because a
 * default there is evaluated once at mount: a dialogue opened *after* the
 * language changed would show the language it was born in.
 */
const cancelText = computed(() => props.cancelLabel ?? t("common.cancel"));
</script>

<template>
  <DialogueShell
    :title="title"
    :danger="danger"
    :size="items && items.length > 0 || preview ? 'm' : 's'"
    :busy="busy"
    @close="emit('cancel')"
  >
    <p class="confirm-message">{{ message }}</p>
    <ul v-if="items && items.length > 0" class="confirm-items">
      <li v-for="(item, index) in items" :key="index">{{ item }}</li>
    </ul>

    <div v-if="preview" class="confirm-preview">
      <strong v-if="previewLabel">{{ previewLabel }}</strong>
      <pre tabindex="0" aria-label="preview">{{ preview }}</pre>
    </div>

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
        {{ cancelText }}
      </button>
      <button
        v-if="secondaryLabel"
        type="button"
        class="btn"
        @click="emit('secondary')"
      >
        {{ secondaryLabel }}
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

.confirm-preview {
  margin-top: var(--md-space-2);
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
}

.confirm-preview pre {
  margin: 0;
  max-height: 220px;
  overflow: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  padding: var(--md-space-2);
  border-radius: var(--md-radius-md);
  background: var(--md-bg-sunken);
  font: var(--md-text-xs)/1.45 var(--md-font-mono);
  user-select: text;
}

.confirm-progress {
  margin-top: var(--md-space-3);
}
</style>
