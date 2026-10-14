<script setup lang="ts">
import { computed } from "vue";

import type { ToolbarVerbModel } from "../components/contracts";
import Icon from "../components/common/Icon.vue";
import SplitAction from "../components/common/SplitAction.vue";
import ProgressTrack from "../components/common/ProgressTrack.vue";

/**
 * `<AppToolbar>` — the global verb row (docs/04 §4): fetch/pull/push/… on the
 * left, quick-open / quick-actions / activity / settings on the right. Verbs
 * disable with a reason (tooltip) when their precondition fails; the toolbar
 * holds no state of its own.
 */
const props = defineProps<{
  verbs: ToolbarVerbModel[];
  activity: {
    count: number;
    busy: boolean;
    stalled: boolean;
    /** True when there is no activity backend — the chip disables with a reason. */
    unavailable?: boolean;
    unavailableReason?: string;
  };
}>();

const emit = defineEmits<{
  run: [id: string];
  choose: [verbId: string, optionId: string];
  openActivity: [];
}>();

const globalVerbs = computed(() => props.verbs.filter((verb) => verb.group === "global"));
const utilityVerbs = computed(() => props.verbs.filter((verb) => verb.group === "utility"));

function run(id: string): void {
  emit("run", id);
}

function choose(verbId: string, optionId: string): void {
  emit("choose", verbId, optionId);
}
</script>

<template>
  <div class="toolbar" role="toolbar" aria-label="Repository actions">
    <div class="verb-group">
      <SplitAction
        v-for="verb in globalVerbs"
        :key="verb.id"
        :label="verb.label"
        :icon="verb.icon"
        :disabled="verb.disabled"
        :reason="verb.reason"
        :busy="verb.busy"
        :options="verb.options"
        compact
        @run="run(verb.id)"
        @choose="choose(verb.id, $event)"
      />
      <span v-if="globalVerbs.length === 0" class="toolbar-empty">No repository actions available.</span>
    </div>

    <span class="toolbar-spacer" />

    <div class="verb-group">
      <button
        v-for="verb in utilityVerbs"
        :key="verb.id"
        type="button"
        class="utility-button"
        :disabled="verb.disabled"
        :title="verb.disabled ? verb.reason : verb.label"
        @click="run(verb.id)"
      >
        <Icon :name="verb.icon" :size="15" />
        <span class="visually-hidden">{{ verb.label }}</span>
      </button>

      <button
        type="button"
        class="activity-chip"
        :class="{ stalled: activity.stalled }"
        aria-label="Open the activity list"
        :disabled="activity.unavailable"
        :title="activity.unavailable ? activity.unavailableReason : 'Activity'"
        @click="emit('openActivity')"
      >
        <ProgressTrack
          v-if="activity.busy"
          :state="activity.stalled ? 'stalled' : 'indeterminate'"
          :phase="activity.stalled ? 'Stalled' : null"
          class="chip-progress"
        />
        <template v-else>
          <Icon name="hourglass" :size="14" />
          <span class="activity-count">{{ activity.count }}</span>
        </template>
      </button>
    </div>
  </div>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  min-width: 0;
}

.verb-group {
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  min-width: 0;
  overflow: hidden;
}

.toolbar-spacer {
  flex: 1;
}

.toolbar-empty {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.utility-button {
  display: inline-flex;
  align-items: center;
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  border-radius: var(--md-radius-sm);
  padding: var(--md-space-1);
}

.utility-button:hover:not(:disabled) {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.activity-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-1);
  border: 1px solid var(--md-border-strong);
  background: var(--md-bg-surface);
  border-radius: var(--md-radius-md);
  padding: 2px var(--md-space-2);
  color: var(--md-fg-muted);
  min-width: 52px;
  justify-content: center;
}

.activity-chip.stalled {
  border-color: var(--md-warning);
  color: var(--md-warning);
}

.activity-count {
  font-size: var(--md-text-xs);
  font-variant-numeric: tabular-nums;
}

.chip-progress {
  min-width: 0;
  width: 60px;
}

.chip-progress :deep(.progress-head) {
  display: none;
}
</style>
