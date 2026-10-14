<script setup lang="ts">
import { computed, inject } from "vue";
import { useI18n } from "vue-i18n";

import type { ErrorPayload } from "../../bridges/bindings";
import { failureIdFor } from "../../diagnostics/failures";
import { OPEN_DIAGNOSTICS, useFailureClaim } from "../../diagnostics/claim";
import { errorText } from "../../i18n";
import EmptyState from "./EmptyState.vue";
import Icon from "./Icon.vue";

/**
 * `<ListShell>` — the standard list chrome (docs/04 §1.2, §6).
 *
 * Every asynchronous list renders exactly four states plus the orthogonal
 * large-content warning. Making it a component means no list can forget a
 * state: the parent passes `status`, the shell renders the arm.
 *
 * The error arm is where a failure becomes visible, so it is also where the
 * shell claims it: while this component is on screen with an error, the failure
 * net stays quiet; when a pane is not mounted, the net's fallback toast is what
 * the user gets instead of a pane that silently says "no commits".
 */
const props = withDefaults(
  defineProps<{
    status: "loading" | "empty" | "error" | "data";
    /** Empty arm. */
    emptyIcon?: string;
    emptyTitle?: string;
    emptyCaption?: string;
    emptyActionLabel?: string;
    /** Error arm: the whole payload, so the shell can translate it and name the code. */
    error?: ErrorPayload | null;
    /** Loading arm text. */
    loadingCaption?: string;
    /** Large-content gate (diff/list): shown as a banner above the content. */
    warning?: string | null;
  }>(),
  {
    emptyIcon: "empty.noFileSelected",
    emptyTitle: "Nothing here",
    loadingCaption: "Loading…",
    error: null,
    warning: null,
  },
);

const emit = defineEmits<{
  retry: [];
  emptyAction: [];
}>();

const { t } = useI18n();
const openDiagnostics = inject(OPEN_DIAGNOSTICS, undefined);

useFailureClaim(computed(() => (props.status === "error" ? props.error : null)));

/** The catalogue sentence for the code (or the class), not the backend's English. */
const headline = computed(() => (props.error ? errorText(props.error) : t("errors.title")));

/** The engine's own words, when they add to the sentence above. */
const reason = computed(() => {
  const message = props.error?.message ?? "";
  return message && message !== headline.value ? message : "";
});

/** What the log and the report can use: code, then the raw detail. */
const detailLine = computed(() => [props.error?.code, props.error?.detail].filter(Boolean).join(" · "));

function onDetails(): void {
  openDiagnostics?.(props.error ? failureIdFor(props.error) : undefined);
}
</script>

<template>
  <div class="list-shell">
    <slot name="toolbar" />
    <div v-if="warning" class="list-warning" role="status">
      <Icon name="exclamationmark.triangle" :size="13" />
      <span class="list-warning-text">{{ warning }}</span>
    </div>

    <div class="list-body">
      <div v-if="status === 'loading'" class="list-note" role="status">
        <span class="spinner" aria-hidden="true" />
        <span>{{ loadingCaption }}</span>
      </div>

      <EmptyState
        v-else-if="status === 'empty'"
        :icon="emptyIcon"
        :title="emptyTitle"
        :caption="emptyCaption"
        :action-label="emptyActionLabel"
        @action="emit('emptyAction')"
      />

      <div v-else-if="status === 'error'" class="list-error" role="alert">
        <Icon name="exclamationmark.circle" :size="18" class="tone-danger" />
        <p class="list-error-message">{{ headline }}</p>
        <p v-if="reason" class="list-error-reason mono">{{ reason }}</p>
        <p v-if="detailLine" class="list-error-detail mono">{{ detailLine }}</p>
        <div class="list-error-actions">
          <button type="button" class="ghost" @click="emit('retry')">{{ t('common.retry') }}</button>
          <button v-if="openDiagnostics" type="button" class="ghost" @click="onDetails">{{ t('diagnostics.actions.details') }}</button>
        </div>
      </div>

      <slot v-else />
    </div>
  </div>
</template>

<style scoped>
.list-shell {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}

.list-warning {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  margin: var(--md-space-2);
  padding: var(--md-space-2) var(--md-space-3);
  border-radius: var(--md-radius-md);
  background: var(--md-bg-warning-soft);
  color: var(--md-fg);
  font-size: var(--md-text-sm);
}

.list-warning :deep(svg) {
  color: var(--md-warning);
}

.list-warning-text {
  flex: 1;
  min-width: 0;
}

.list-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.list-note {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--md-space-2);
  padding: var(--md-space-4);
  color: var(--md-fg-muted);
  font-size: var(--md-text-sm);
}

.spinner {
  width: 13px;
  height: 13px;
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

.list-error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-5) var(--md-space-4);
  text-align: center;
}

.list-error-message {
  margin: 0;
  font-size: var(--md-text-sm);
  max-width: 420px;
}

.list-error-reason,
.list-error-detail {
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
  max-width: 420px;
  overflow-wrap: anywhere;
  user-select: text;
}

.list-error-actions {
  display: flex;
  gap: var(--md-space-3);
}

.ghost {
  border: none;
  background: transparent;
  color: var(--md-accent);
  padding: var(--md-space-1) var(--md-space-2);
}

.ghost:hover:not(:disabled) {
  background: transparent;
  text-decoration: underline;
}
</style>
