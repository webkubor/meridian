<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { CompareEntry, Comparison, LoadState, RefChoice } from "../contracts";
import Icon from "../common/Icon.vue";
import ListShell from "../common/ListShell.vue";
import MasterDetail from "../../layout/MasterDetail.vue";
import VirtualList from "../common/VirtualList.vue";

/**
 * `<ComparePane>` — the compare screen (docs/04 §S9/§S13): two ref pickers
 * with a swap, the merge-base line, the ahead/behind counts, and the range's
 * commits split into the two sides. Selecting a commit reveals it in the
 * detail column (the screen supplies the commit-detail pane through the slot).
 */
const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    state: LoadState<Comparison>;
    comparison: Comparison | null;
    left: RefChoice | null;
    right: RefChoice | null;
    selectedOid?: string | null;
    detailWidth: number;
    /** Set while a comparison request is in flight. */
    busy?: boolean;
    /**
     * Non-null when the compare backend is not built: the ref pickers and the
     * swap disable, with this text as the reason. Hides nothing, invents
     * nothing.
     */
    unavailable?: string | null;
  }>(),
  { selectedOid: null, busy: false, unavailable: null },
);

const emit = defineEmits<{
  chooseRef: [side: "left" | "right"];
  swap: [];
  selectCommit: [oid: string];
  openCommit: [oid: string];
  "update:detailWidth": [value: number];
  refresh: [];
}>();

const entries = computed(() => props.comparison?.entries ?? []);

const aheadEntries = computed(() => entries.value.filter((entry) => entry.side === "ahead"));
const behindEntries = computed(() => entries.value.filter((entry) => entry.side === "behind"));

function orderOf(entry: CompareEntry): number {
  return entries.value.indexOf(entry);
}

const baseLabel = computed(() => {
  const base = props.comparison?.mergeBase;
  return base ? base.slice(0, 7) : t("compare.noCommonAncestor");
});
</script>

<template>
  <div class="compare-pane">
    <header class="compare-bar">
      <button
        type="button"
        class="ref-picker"
        :disabled="busy || !!unavailable"
        :title="unavailable ?? undefined"
        @click="emit('chooseRef', 'left')"
      >
        <span class="ref-side">{{ t("compare.baseSide") }}</span>
        <span class="ref-name">{{ left?.label ?? t("compare.chooseRef") }}</span>
        <Icon name="chevron.down" :size="11" />
      </button>

      <button
        type="button"
        class="swap"
        :title="unavailable ?? t('compare.swapTitle')"
        :disabled="busy || !!unavailable"
        @click="emit('swap')"
      >
        <Icon name="arrow.left.arrow.right" :size="13" />
      </button>

      <button
        type="button"
        class="ref-picker"
        :disabled="busy || !!unavailable"
        :title="unavailable ?? undefined"
        @click="emit('chooseRef', 'right')"
      >
        <span class="ref-side">{{ t("compare.compareSide") }}</span>
        <span class="ref-name">{{ right?.label ?? t("compare.chooseRef") }}</span>
        <Icon name="chevron.down" :size="11" />
      </button>

      <span class="compare-spacer" />

      <span v-if="comparison" class="compare-counts">
        <!-- Ahead/behind wording lives under `branch.*` on purpose: it is one
             concept shown in two panes, and two translations would drift. -->
        <span class="count-ahead">
          <Icon name="arrow.up" :size="11" />
          {{ t("branch.aheadBy", { count: comparison.ahead }) }}
        </span>
        <span class="count-behind">
          <Icon name="arrow.down" :size="11" />
          {{ t("branch.behindBy", { count: comparison.behind }) }}
        </span>
        <span class="base-line">{{ t("compare.mergeBase", { base: baseLabel }) }}</span>
      </span>
    </header>

    <ListShell
      :status="state.status"
      empty-icon="empty.noCommitSelected"
      :empty-title="t('compare.emptyTitle')"
      :empty-caption="t('compare.emptyCaption')"
      :error="state.status === 'error' ? state.error : null"
      :loading-caption="t('compare.loading')"
      @retry="emit('refresh')"
    >
      <MasterDetail
        :detail-width="detailWidth"
        :min="320"
        :max="900"
        @update:detail-width="emit('update:detailWidth', $event)"
      >
        <template #master>
          <div class="range-list">
            <p v-if="aheadEntries.length > 0" class="range-heading">
              <Icon name="arrow.up" :size="11" />
              {{ t("compare.onlyIn", { ref: right?.label ?? t("compare.targetFallback") }) }}
            </p>
            <p v-if="behindEntries.length > 0" class="range-heading">
              <Icon name="arrow.down" :size="11" />
              {{ t("compare.onlyIn", { ref: left?.label ?? t("compare.baseFallback") }) }}
            </p>

            <VirtualList :count="entries.length" :row-height="30" class="range-rows">
              <template #default="{ index }">
                <button
                  type="button"
                  class="range-row"
                  :class="{ selected: entries[index].commit.oid === selectedOid }"
                  :aria-selected="entries[index].commit.oid === selectedOid"
                  @click="emit('selectCommit', entries[index].commit.oid)"
                  @dblclick="emit('openCommit', entries[index].commit.oid)"
                >
                  <span class="range-side" :class="`side-${entries[index].side}`">
                    <Icon :name="entries[index].side === 'ahead' ? 'arrow.up' : 'arrow.down'" :size="11" />
                  </span>
                  <span class="range-body">
                    <span class="range-summary">{{ entries[index].commit.summary }}</span>
                    <span class="range-meta">
                      <span class="mono">{{ entries[index].commit.shortOid }}</span>
                      <span class="range-dot" aria-hidden="true">·</span>
                      {{ entries[index].commit.author.name }}
                    </span>
                  </span>
                  <span class="range-order">{{ orderOf(entries[index]) + 1 }}</span>
                </button>
              </template>
            </VirtualList>
          </div>
        </template>

        <template #detail>
          <slot name="detail" />
        </template>
      </MasterDetail>
    </ListShell>
  </div>
</template>

<style scoped>
.compare-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

.compare-bar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: var(--md-toolbar-height);
  padding: 0 var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
  background: var(--md-bg-header);
}

.ref-picker {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-2);
  border: 1px solid var(--md-border-strong);
  border-radius: var(--md-radius-md);
  background: var(--md-bg-surface);
  padding: 2px var(--md-space-2);
  min-width: 180px;
  max-width: 300px;
}

.ref-side {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.ref-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--md-text-sm);
}

.swap {
  display: inline-flex;
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  padding: var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.swap:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.compare-spacer {
  flex: 1;
}

.compare-counts {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}

.count-ahead,
.count-behind {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.count-ahead {
  color: var(--md-success);
}

.count-behind {
  color: var(--md-info);
}

.base-line {
  color: var(--md-fg-subtle);
}

.range-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
  overflow: hidden;
}

.range-heading {
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  margin: 0;
  padding: var(--md-space-1) var(--md-space-3);
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
  background: var(--md-bg-header);
  border-bottom: 1px solid var(--md-border-inactive);
}

.range-rows {
  flex: 1;
  min-height: 0;
}

.range-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  height: 30px;
  padding: 0 var(--md-space-3);
  border: none;
  border-radius: 0;
  background: transparent;
  text-align: left;
}

.range-row:hover {
  background: var(--md-bg-hover);
}

.range-row.selected {
  background: var(--md-bg-selected);
}

.range-side {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: var(--md-radius-sm);
  flex: none;
}

.range-side.side-ahead {
  color: var(--md-success);
  background: var(--md-bg-success-soft);
}

.range-side.side-behind {
  color: var(--md-info);
  background: var(--md-bg-info-soft);
}

.range-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.range-summary {
  font-size: var(--md-text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.range-meta {
  display: inline-flex;
  gap: var(--md-space-1);
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.range-order {
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
  font-variant-numeric: tabular-nums;
}
</style>
