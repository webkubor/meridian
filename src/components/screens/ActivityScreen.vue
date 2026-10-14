<script setup lang="ts">
/**
 * `<ActivityScreen>` — the operations recorded from this window's actions.
 *
 * Presentational: it renders entries and emits `clear`. The list lives in
 * `useActivity`, which is also what the toolbar chip counts, so those two views
 * cannot disagree.
 *
 * An entry means its label and outcome are known. It does not pretend to have
 * byte-level progress unless the underlying operation actually reports it.
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { ActivityEntry } from "../../composables/useActivity";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import RowBadge from "../common/RowBadge.vue";

const props = defineProps<{
  entries: ActivityEntry[];
  running: number;
}>();

const emit = defineEmits<{ clear: [] }>();
const { t } = useI18n();

const finished = computed(() => props.entries.filter((entry) => entry.state !== "running"));

function labelOf(entry: ActivityEntry): string {
  return entry.label ?? t(entry.labelKey);
}

function since(entry: ActivityEntry): string {
  const format = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const end = entry.finishedAt ?? Date.now();
  return `${format.format(new Date(entry.startedAt))} → ${format.format(new Date(end))}`;
}
</script>

<template>
  <section class="activity">
    <header class="activity-head">
      <div>
        <h2 class="activity-title">{{ $t("shell.activity.title") }}</h2>
        <p class="activity-subtitle">
          {{ running > 0 ? $t("shell.activity.running", { count: running }) : $t("shell.activity.idle") }}
        </p>
      </div>
      <button
        type="button"
        class="btn"
        :disabled="finished.length === 0"
        :title="finished.length === 0 ? $t('shell.activity.nothingToClear') : $t('shell.activity.clear')"
        @click="emit('clear')"
      >
        {{ $t("shell.activity.clear") }}
      </button>
    </header>

    <EmptyState
      v-if="entries.length === 0"
      icon="empty.tray"
      :title="$t('shell.activity.empty')"
      :caption="$t('shell.activity.emptyCaption')"
    />

    <ul v-else class="activity-list">
      <li v-for="entry in entries" :key="entry.id" class="activity-row">
        <span class="state-dot" :class="entry.state" aria-hidden="true" />
        <span class="body">
          <span class="name">{{ labelOf(entry) }}</span>
          <span class="time mono">{{ since(entry) }}</span>
          <span v-if="entry.error" class="error">{{ entry.error }}</span>
        </span>
        <RowBadge
          :label="entry.state === 'running'
            ? $t('shell.activity.state.running')
            : entry.state === 'done'
              ? $t('shell.activity.state.done')
              : $t('shell.activity.state.failed')"
          :icon="entry.state === 'running'
            ? 'arrow.clockwise'
            : entry.state === 'done'
              ? 'checkmark'
              : 'exclamationmark.triangle'"
          :tone="entry.state === 'failed' ? 'danger' : entry.state === 'done' ? 'positive' : 'info'"
        />
      </li>
    </ul>

    <footer class="activity-foot">
      <Icon name="info.circle" :size="12" />
      <span>{{ $t("shell.activity.privacy") }}</span>
    </footer>
  </section>
</template>

<style scoped>
.activity {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-3);
  height: 100%;
  min-height: 0;
  padding: var(--md-space-5) var(--md-space-6);
  overflow: auto;
}

.activity-head {
  display: flex;
  align-items: flex-start;
  gap: var(--md-space-3);
}

.activity-head > div {
  flex: 1;
  min-width: 0;
}

.activity-title {
  margin: 0;
  font-size: var(--md-text-xl);
  font-weight: 600;
}

.activity-subtitle {
  margin: var(--md-space-1) 0 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg-muted);
}

.activity-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
  background: var(--md-border-inactive);
  border: 1px solid var(--md-border);
  border-radius: var(--md-radius-lg);
  overflow: hidden;
}

.activity-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-3);
  padding: var(--md-space-2) var(--md-space-3);
  background: color-mix(in oklch, var(--md-bg-surface) 88%, transparent);
}

.state-dot {
  flex: none;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--md-fg-subtle);
}

.state-dot.running {
  background: var(--md-info);
  animation: md-pulse 1.4s ease-in-out infinite;
}

.state-dot.done {
  background: var(--md-success);
}

.state-dot.failed {
  background: var(--md-danger);
}

@keyframes md-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.35; }
}

@media (prefers-reduced-motion: reduce) {
  .state-dot.running { animation: none; opacity: 1; }
}

.body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.name { font-size: var(--md-text-md); }
.time { font-size: var(--md-text-xs); color: var(--md-fg-subtle); }
.error { font-size: var(--md-text-xs); color: var(--md-danger); }

.activity-foot {
  display: flex;
  align-items: flex-start;
  gap: var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}
</style>