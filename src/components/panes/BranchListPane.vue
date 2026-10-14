<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { BranchFilter } from "../../bridges/bindings";
import type { BranchReviewInfo, LoadState } from "../contracts";
import Icon from "../common/Icon.vue";
import ListShell from "../common/ListShell.vue";
import RowBadge from "../common/RowBadge.vue";
import SearchField from "../common/SearchField.vue";
import VirtualList from "../common/VirtualList.vue";

/**
 * `<BranchListPane>` — the branch tab's list (docs/04 §S9): local/remote/all
 * filter, search, and one row per branch with its tracking badges (ahead,
 * behind, fully-merged). Selection and every verb are reported upward.
 */
const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    state: LoadState<BranchReviewInfo[]>;
    branches: BranchReviewInfo[];
    selectedName?: string | null;
    filter: BranchFilter;
    search: string;
    /** The branch whose HEAD the current one is on (for merge-into labels). */
    currentBranch?: string | null;
  }>(),
  { selectedName: null, currentBranch: null },
);

const emit = defineEmits<{
  select: [name: string];
  activate: [name: string];
  contextmenu: [name: string, event: MouseEvent];
  create: [];
  checkout: [name: string];
  merge: [name: string];
  rebase: [name: string];
  delete: [name: string];
  rename: [name: string];
  "update:filter": [filter: BranchFilter];
  "update:search": [value: string];
  refresh: [];
}>();

const FILTERS: { id: BranchFilter; label: string }[] = [
  { id: "local", label: "branch.filterLocal" },
  { id: "remote", label: "branch.filterRemote" },
  { id: "all", label: "branch.filterAll" },
];

const filtered = computed(() => {
  const needle = props.search.trim().toLowerCase();
  return props.branches.filter((branch) => {
    if (props.filter === "local" && branch.isRemote) return false;
    if (props.filter === "remote" && !branch.isRemote) return false;
    return needle === "" || branch.name.toLowerCase().includes(needle);
  });
});

function isCurrent(branch: BranchReviewInfo): boolean {
  return branch.isHead;
}

function symbolUpstream(branch: BranchReviewInfo): string {
  return t("branch.tracking", { ref: branch.upstream ?? branch.name });
}
</script>

<template>
  <ListShell
    :status="state.status"
    empty-icon="empty.noBranchSelected"
    :empty-title="t('branch.emptyTitle')"
    :empty-caption="t('branch.emptyCaption')"
    :empty-action-label="t('branch.emptyAction')"
    :error-message="state.status === 'error' ? state.error.message : null"
    :loading-caption="t('branch.loading')"
    @retry="emit('refresh')"
    @empty-action="emit('create')"
  >
    <template #toolbar>
      <div class="branch-toolbar">
        <div class="branch-filter" role="radiogroup" :aria-label="t('branch.scopeLabel')">
          <button
            v-for="option in FILTERS"
            :key="option.id"
            type="button"
            role="radio"
            :aria-checked="filter === option.id"
            class="filter-tab"
            :class="{ active: filter === option.id }"
            @click="emit('update:filter', option.id)"
          >
            {{ t(option.label) }}
          </button>
        </div>

        <SearchField
          :model-value="search"
          :label="t('branch.searchLabel')"
          :placeholder="t('branch.searchPlaceholder')"
          class="branch-search"
          @update:model-value="emit('update:search', $event)"
        />

        <span class="branch-spacer" />
        <button type="button" class="branch-new" @click="emit('create')">
          <Icon name="plus" :size="13" />
          {{ t("branch.newBranch") }}
        </button>
      </div>
    </template>

    <VirtualList :count="filtered.length" :row-height="26" class="branch-list">
      <template #default="{ index }">
        <div
          class="branch-row"
          :class="{ selected: filtered[index].fullName === selectedName, head: isCurrent(filtered[index]) }"
          role="option"
          :aria-selected="filtered[index].fullName === selectedName"
          @click="emit('select', filtered[index].fullName)"
          @dblclick="emit('activate', filtered[index].fullName)"
          @contextmenu.prevent="emit('contextmenu', filtered[index].fullName, $event)"
        >
          <Icon
            :name="filtered[index].isRemote ? 'cloud' : 'arrow.triangle.branch'"
            :size="13"
            class="branch-icon"
          />
          <span class="branch-name">{{ filtered[index].name }}</span>
          <span v-if="isCurrent(filtered[index])" class="branch-head">
            <Icon name="checkmark.circle.fill" :size="11" />
            <!-- HEAD is a git ref, not a word: it stays literal in every locale. -->
            HEAD
          </span>
          <span class="branch-spacer" />
          <span v-if="filtered[index].upstream" class="branch-upstream" :title="symbolUpstream(filtered[index])">
            {{ filtered[index].upstream }}
          </span>
          <RowBadge
            v-if="filtered[index].ahead > 0"
            :label="filtered[index].ahead"
            icon="arrow.up"
            tone="positive"
          />
          <RowBadge
            v-if="filtered[index].behind > 0"
            :label="filtered[index].behind"
            icon="arrow.down"
            tone="info"
          />
          <RowBadge
            v-if="filtered[index].fullyMerged"
            icon="checkmark.circle"
            tone="muted"
          />
        </div>
      </template>
    </VirtualList>
  </ListShell>
</template>

<style scoped>
.branch-toolbar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: var(--md-toolbar-height);
  padding: 0 var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
}

.branch-filter {
  display: inline-flex;
  gap: 2px;
}

.filter-tab {
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  font-size: var(--md-text-xs);
  padding: 2px var(--md-space-2);
  border-radius: var(--md-radius-sm);
}

.filter-tab.active {
  background: var(--md-bg-selected);
  color: var(--md-fg);
}

.branch-search {
  flex: 1;
  max-width: 320px;
}

.branch-spacer {
  flex: 1;
}

.branch-new {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 1px solid var(--md-border-strong);
  background: var(--md-bg-surface);
  border-radius: var(--md-radius-md);
  padding: 2px var(--md-space-2);
  font-size: var(--md-text-xs);
}

.branch-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  height: 26px;
  padding: 0 var(--md-space-2);
  font-size: var(--md-text-sm);
}

.branch-row:hover {
  background: var(--md-bg-hover);
}

.branch-row.selected {
  background: var(--md-bg-selected);
}

.branch-icon {
  color: var(--md-fg-subtle);
}

.branch-row.head .branch-icon {
  color: var(--md-accent);
}

.branch-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 50%;
}

.branch-head {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: var(--md-text-xs);
  color: var(--md-accent);
}

.branch-upstream {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 180px;
}
</style>
