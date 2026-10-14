<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { CommitInfo } from "../../bridges/bindings";
import type {
  FilterSectionModel,
  GraphLayout,
  HistorySearchModel,
  HistoryViewOptions,
  LoadState,
} from "../contracts";
import FilterMenu from "../common/FilterMenu.vue";
import ListShell from "../common/ListShell.vue";
import SearchField from "../common/SearchField.vue";
import { VirtualCommitList } from "../commit-list";

/**
 * `<HistoryPane>` — the history tab (docs/04 §S8): the view-settings menu
 * (graph on/off, density, grouping, first-parent), the search bar, and the
 * virtualised commit list with its graph column. The heavy lifting (row model,
 * graph painting, paging) is the commit-list module's; this pane owns the
 * chrome, the filter model, and the four-state shell.
 */
const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    state: LoadState<CommitInfo[]>;
    commits: CommitInfo[];
    layout: GraphLayout | null;
    /** Bubble lane per commit, used when `layout.lanes` is absent. */
    graphRows?: { lane: number | null }[] | null;
    view: HistoryViewOptions;
    search: HistorySearchModel;
    selectedOid?: string | null;
    headOid?: string | null;
    truncated?: boolean;
    loadingMore?: boolean;
    /** Set when the pane starts mid-history (compare/range). */
    hasMoreAbove?: boolean;
    forkPoints?: string[];
    remotes?: string[];
    /** Sort dropdown is disabled while a search is narrowing results. */
    searchActive?: boolean;
    /**
     * When set, the merge-status select is disabled and shows this as its title:
     * the engine has no merge predicate to filter on.
     */
    typeFilterUnavailable?: string | null;
  }>(),
  {
    graphRows: null,
    selectedOid: null,
    headOid: null,
    truncated: false,
    loadingMore: false,
    hasMoreAbove: false,
    forkPoints: () => [],
    remotes: () => [],
    searchActive: false,
    typeFilterUnavailable: null,
  },
);

const emit = defineEmits<{
  select: [commit: CommitInfo];
  activate: [commit: CommitInfo];
  needMore: [];
  refClick: [payload: { commit: CommitInfo; ref: string; label: string }];
  "update:view": [view: HistoryViewOptions];
  "update:search": [search: HistorySearchModel];
  refresh: [];
}>();

// Option labels are held as catalogue keys, not resolved strings: the select
// and the filter menu both read them inside render/computed context, so a
// language switch has to pick them up without a reload.
const TYPE_OPTIONS: { id: HistorySearchModel["type"]; label: string }[] = [
  { id: "all", label: "history.typeAll" },
  { id: "merges", label: "history.typeMerges" },
  { id: "nonMerges", label: "history.typeNonMerges" },
];

const filterSections = computed<FilterSectionModel[]>(() => [
  {
    id: "view",
    label: t("history.sectionView"),
    kind: "check",
    options: [
      { id: "showGraph", label: t("history.showGraph"), checked: props.view.showGraph },
      { id: "firstParent", label: t("history.firstParentOnly"), checked: props.view.firstParent },
    ],
  },
  {
    id: "density",
    label: t("history.sectionDensity"),
    kind: "radio",
    options: [
      { id: "density:compact", label: t("history.densityCompact"), checked: props.view.density === "compact" },
      { id: "density:regular", label: t("history.densityRegular"), checked: props.view.density === "regular" },
    ],
  },
  {
    id: "grouping",
    label: t("history.sectionGrouping"),
    kind: "radio",
    options: [
      { id: "grouping:off", label: t("history.groupingOff"), checked: props.view.dateGrouping === "off" },
      { id: "grouping:day", label: t("history.groupingDay"), checked: props.view.dateGrouping === "day" },
      { id: "grouping:week", label: t("history.groupingWeek"), checked: props.view.dateGrouping === "week" },
      { id: "grouping:month", label: t("history.groupingMonth"), checked: props.view.dateGrouping === "month" },
    ],
  },
  {
    id: "sort",
    label: t("history.sectionSort"),
    kind: "radio",
    options: [
      { id: "sort:date", label: t("history.sortDate"), checked: props.view.sort === "date" },
      { id: "sort:topo", label: t("history.sortTopo"), checked: props.view.sort === "topo" },
    ],
  },
]);

function toggleOption(optionId: string): void {
  const next: HistoryViewOptions = { ...props.view };
  if (optionId === "showGraph") next.showGraph = !next.showGraph;
  else if (optionId === "firstParent") next.firstParent = !next.firstParent;
  else if (optionId.startsWith("density:")) next.density = optionId.slice(8) as HistoryViewOptions["density"];
  else if (optionId.startsWith("grouping:")) {
    next.dateGrouping = optionId.slice(9) as HistoryViewOptions["dateGrouping"];
  } else if (optionId.startsWith("sort:")) next.sort = optionId.slice(5) as HistoryViewOptions["sort"];
  emit("update:view", next);
}

function resetView(): void {
  emit("update:view", {
    showGraph: true,
    density: "regular",
    sort: "date",
    firstParent: false,
    dateGrouping: "day",
  });
}

function patchSearch(patch: Partial<HistorySearchModel>): void {
  emit("update:search", { ...props.search, ...patch });
}
</script>

<template>
  <ListShell
    :status="state.status"
    empty-icon="empty.noCommitSelected"
    :empty-title="t('history.emptyTitle')"
    :empty-caption="t('history.emptyCaption')"
    :error="state.status === 'error' ? state.error : null"
    :loading-caption="t('history.loading')"
    @retry="emit('refresh')"
  >
    <template #toolbar>
      <div class="history-toolbar">
        <SearchField
          :model-value="search.text"
          :label="t('history.searchLabel')"
          :placeholder="t('history.searchPlaceholder')"
          class="history-search"
          @update:model-value="patchSearch({ text: $event })"
        />

        <label class="search-type">
          <span class="visually-hidden">{{ t("history.typeLabel") }}</span>
          <select
            :value="search.type"
            :disabled="searchActive || !!typeFilterUnavailable"
            :title="typeFilterUnavailable ?? undefined"
            @change="patchSearch({ type: ($event.target as HTMLSelectElement).value as HistorySearchModel['type'] })"
          >
            <option v-for="option in TYPE_OPTIONS" :key="option.id" :value="option.id">
              {{ t(option.label) }}
            </option>
          </select>
        </label>

        <button
          v-if="search.text || search.author"
          type="button"
          class="history-clear"
          @click="patchSearch({ text: '', author: '' })"
        >
          {{ t("history.clear") }}
        </button>

        <span class="history-spacer" />
        <FilterMenu
          :label="t('history.viewSettings')"
          :sections="filterSections"
          @toggle="toggleOption"
          @reset="resetView"
        />
      </div>
    </template>

    <VirtualCommitList
      :commits="commits"
      :layout="layout"
      :graph-rows="graphRows"
      :density="view.density"
      :date-grouping="view.dateGrouping"
      :selected-oid="selectedOid"
      :head-oid="headOid"
      :fork-points="forkPoints"
      :remotes="remotes"
      :show-graph="view.showGraph"
      :loading="state.status === 'loading'"
      :loading-more="loadingMore"
      :has-more="truncated"
      :has-more-above="hasMoreAbove"
      @select="emit('select', $event)"
      @activate="emit('activate', $event)"
      @ref-click="emit('refClick', $event)"
      @need-more="emit('needMore')"
    />
  </ListShell>
</template>

<style scoped>
.history-toolbar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: var(--md-toolbar-height);
  padding: 0 var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
}

.history-search {
  flex: 1;
  min-width: 160px;
  max-width: 420px;
}

.search-type select {
  height: 24px;
  font-size: var(--md-text-xs);
  padding: 0 var(--md-space-1);
}

.history-clear {
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  font-size: var(--md-text-xs);
}

.history-clear:hover {
  color: var(--md-accent);
  background: transparent;
}

.history-spacer {
  flex: 1;
}
</style>
