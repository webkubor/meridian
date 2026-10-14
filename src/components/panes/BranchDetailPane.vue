<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { BranchReviewInfo, SplitActionOption } from "../contracts";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import RowBadge from "../common/RowBadge.vue";
import SplitAction from "../common/SplitAction.vue";

/**
 * `<BranchDetailPane>` — the branch tab's detail column (docs/04 §S9): the
 * branch's identity, its tracking state, and the verb set (checkout, merge into
 * the current branch, rebase onto it, push/pull, rename, delete). Verbs that
 * need a dialogue — rename, delete — are opened by the shell, which owns the
 * confirm/prompt steps.
 */
const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    branch: BranchReviewInfo | null;
    /** Name of the branch the user is on now (for "Merge into …"). */
    currentBranch?: string | null;
    /** Disables the destructive verbs (protected/current branch). */
    canDelete?: boolean;
    busy?: boolean;
    pushOptions?: SplitActionOption[];
  }>(),
  { currentBranch: null, canDelete: true, busy: false, pushOptions: undefined },
);

const emit = defineEmits<{
  checkout: [];
  mergeIntoCurrent: [];
  rebaseOnto: [];
  push: [];
  pushChoose: [optionId: string];
  pull: [];
  rename: [];
  remove: [];
  showInHistory: [];
  openCompare: [];
  copyName: [];
}>();

const upstream = computed(() => props.branch?.upstream ?? null);
const isCurrent = computed(() => props.branch?.isHead ?? false);
const remote = computed(() => props.branch?.isRemote ?? false);

const mergeLabel = computed(() =>
  props.currentBranch
    ? t("branch.mergeInto", { name: props.currentBranch })
    : t("branch.mergeIntoCurrent"),
);

const deleteReason = computed(() => {
  if (!props.branch) return undefined;
  if (isCurrent.value) return t("branch.reasonCheckedOut");
  if (!props.canDelete) return t("branch.reasonProtected");
  return undefined;
});
</script>

<template>
  <div class="branch-detail">
    <EmptyState
      v-if="!branch"
      icon="empty.noBranchSelected"
      :title="t('branch.noSelectionTitle')"
      :caption="t('branch.noSelectionCaption')"
    />

    <template v-else>
      <header class="branch-header">
        <Icon :name="remote ? 'cloud' : 'arrow.triangle.branch'" :size="16" class="branch-kind" />
        <h2 class="branch-title">{{ branch.name }}</h2>
        <!-- HEAD is a git ref; "Merged" is prose. -->
        <RowBadge v-if="isCurrent" label="HEAD" tone="info" />
        <RowBadge v-if="branch.fullyMerged" :label="t('branch.merged')" icon="checkmark.circle" tone="muted" />
        <span class="branch-spacer" />
        <button type="button" class="verb ghost" :title="t('branch.copyName')" @click="emit('copyName')">
          <Icon name="doc.on.doc" :size="13" />
        </button>
      </header>

      <dl class="branch-facts">
        <div class="fact">
          <dt>{{ t("branch.factTip") }}</dt>
          <dd class="mono">{{ branch.oid.slice(0, 7) }} · {{ branch.tipSummary }}</dd>
        </div>
        <div class="fact">
          <dt>{{ t("branch.factUpstream") }}</dt>
          <dd v-if="upstream" class="mono">{{ upstream }}</dd>
          <dd v-else class="muted">{{ t("branch.notTracking") }}</dd>
        </div>
        <div v-if="branch.lastAuthor" class="fact">
          <dt>{{ t("branch.factLastAuthor") }}</dt>
          <dd>{{ branch.lastAuthor }}</dd>
        </div>
        <div class="fact">
          <dt>{{ t("branch.factTracking") }}</dt>
          <dd class="tracking">
            <span class="track up">
              <Icon name="arrow.up" :size="11" />{{ t("branch.aheadBy", { count: branch.ahead }) }}
            </span>
            <span class="track down">
              <Icon name="arrow.down" :size="11" />{{ t("branch.behindBy", { count: branch.behind }) }}
            </span>
          </dd>
        </div>
      </dl>

      <div class="branch-actions">
        <button
          v-if="!isCurrent"
          type="button"
          class="verb primary"
          :disabled="busy"
          @click="emit('checkout')"
        >
          <Icon name="arrow.triangle.branch" :size="13" />
          {{ t("branch.checkout") }}
        </button>
        <span v-else class="verb current-flag">
          <Icon name="checkmark.circle.fill" :size="13" />
          {{ t("branch.checkedOut") }}
        </span>

        <template v-if="!remote">
          <button type="button" class="verb" :disabled="busy" @click="emit('mergeIntoCurrent')">
            <Icon name="arrow.triangle.merge" :size="13" />
            {{ mergeLabel }}
          </button>
          <button type="button" class="verb" :disabled="busy" @click="emit('rebaseOnto')">
            <Icon name="arrow.triangle.2.circlepath" :size="13" />
            {{ t("branch.rebaseOnto") }}
          </button>
        </template>

        <SplitAction
          v-if="!remote"
          :label="t('branch.push')"
          icon="arrow.up.circle"
          :busy="busy"
          :options="pushOptions"
          @run="emit('push')"
          @choose="emit('pushChoose', $event)"
        />
        <button v-else type="button" class="verb" :disabled="busy" @click="emit('pull')">
          <Icon name="tray.and.arrow.down" :size="13" />
          {{ t("branch.fetchIntoLocal") }}
        </button>

        <span class="branch-spacer" />

        <button type="button" class="verb" @click="emit('showInHistory')">
          <Icon name="clock" :size="13" />
          {{ t("branch.showInHistory") }}
        </button>
        <button type="button" class="verb" @click="emit('openCompare')">
          <Icon name="arrow.left.arrow.right" :size="13" />
          {{ t("branch.compare") }}
        </button>
        <button type="button" class="verb" :disabled="busy" @click="emit('rename')">
          <Icon name="pencil" :size="13" />
          {{ t("branch.rename") }}
        </button>
        <button
          type="button"
          class="verb danger"
          :disabled="busy || !canDelete || isCurrent"
          :title="deleteReason"
          @click="emit('remove')"
        >
          <Icon name="trash" :size="13" />
          {{ t("branch.delete") }}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.branch-detail {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-3);
  padding: var(--md-space-3);
  min-height: 0;
  flex: 1;
  overflow-y: auto;
}

.branch-header {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
}

.branch-kind {
  color: var(--md-accent);
}

.branch-title {
  margin: 0;
  font-size: var(--md-text-lg);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.branch-spacer {
  flex: 1;
}

.branch-facts {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
  padding: var(--md-space-2);
  background: var(--md-bg-sunken);
  border-radius: var(--md-radius-md);
}

.fact {
  display: flex;
  gap: var(--md-space-2);
  font-size: var(--md-text-sm);
}

.fact dt {
  width: 90px;
  flex: none;
  color: var(--md-fg-subtle);
  font-size: var(--md-text-xs);
  padding-top: 2px;
}

.fact dd {
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fact dd.muted {
  color: var(--md-fg-disabled);
}

.tracking {
  display: inline-flex;
  gap: var(--md-space-3);
}

.track {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: var(--md-text-xs);
}

.track.up {
  color: var(--md-success);
}

.track.down {
  color: var(--md-info);
}

.branch-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--md-space-2);
}

.verb {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 1px solid var(--md-border-strong);
  background: var(--md-bg-surface);
  border-radius: var(--md-radius-md);
  padding: 2px var(--md-space-2);
  font-size: var(--md-text-sm);
  color: var(--md-fg);
}

.verb:hover:not(:disabled) {
  background: var(--md-bg-hover);
}

.verb.ghost {
  border-color: transparent;
  background: transparent;
  color: var(--md-fg-muted);
}

.verb.primary {
  background: var(--md-accent);
  border-color: var(--md-accent);
  color: var(--md-accent-contrast);
}

.verb.danger:hover:not(:disabled) {
  color: var(--md-danger);
  border-color: var(--md-danger);
}

.verb.current-flag {
  border-color: transparent;
  color: var(--md-accent);
  font-weight: 600;
}

.verb:disabled {
  opacity: 0.55;
}
</style>
