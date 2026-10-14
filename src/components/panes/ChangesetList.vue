<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { Changeset, ChangesetEntry } from "../contracts";
import type { UiStatusView } from "../../bridges/bindings";
import StatusGlyph from "../common/StatusGlyph.vue";
import VirtualList from "../common/VirtualList.vue";

/**
 * `<ChangesetList>` — the files of one commit / comparison side (docs/04 §S8).
 * A flat, virtualised list of `path  +A −D`, each row carrying its status
 * glyph. Renames show `old → new`; the row reports selection upward.
 */
const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    changeset: Changeset;
    selectedPath?: string | null;
    /** Header row with the file count and totals. */
    showSummary?: boolean;
  }>(),
  { selectedPath: null, showSummary: true },
);

const emit = defineEmits<{
  select: [path: string, event: MouseEvent];
  contextmenu: [path: string, event: MouseEvent];
  activate: [path: string];
}>();

/**
 * `short` stays the git porcelain letter in every language — it is the glyph
 * users already read out of `git status` — while `text` is prose and therefore
 * localised at render time.
 */
const VIEW_BY_STATUS: Record<ChangesetEntry["status"], Omit<UiStatusView, "text"> & { text: string }> = {
  added: { short: "A", text: "commit.statusAdded", tone: "positive" },
  copied: { short: "C", text: "commit.statusCopied", tone: "positive" },
  deleted: { short: "D", text: "commit.statusDeleted", tone: "danger" },
  modified: { short: "M", text: "commit.statusModified", tone: "info" },
  renamed: { short: "R", text: "commit.statusRenamed", tone: "positive" },
  unmerged: { short: "U", text: "commit.statusUnmerged", tone: "danger" },
  untracked: { short: "?", text: "commit.statusUntracked", tone: "muted" },
  ignored: { short: "!", text: "commit.statusIgnored", tone: "muted" },
};

function statusView(status: ChangesetEntry["status"]): UiStatusView {
  const view = VIEW_BY_STATUS[status];
  return { short: view.short, text: t(view.text), tone: view.tone };
}

const entries = computed(() => props.changeset.entries);

function labelOf(entry: ChangesetEntry): string {
  if (entry.oldPath && entry.oldPath !== entry.path) return `${entry.oldPath} → ${entry.path}`;
  return entry.path;
}
</script>

<template>
  <div class="changeset">
    <header v-if="showSummary" class="changeset-summary">
      <span class="summary-files">{{ t("commit.filesChanged", { count: changeset.stats.files }, changeset.stats.files) }}</span>
      <span class="summary-add">+{{ changeset.stats.additions }}</span>
      <span class="summary-del">−{{ changeset.stats.deletions }}</span>
    </header>

    <VirtualList :count="entries.length" :row-height="24" class="changeset-list">
      <template #default="{ index }">
        <div
          class="changeset-row"
          :class="{ selected: entries[index].path === selectedPath }"
          role="option"
          :aria-selected="entries[index].path === selectedPath"
          @click="emit('select', entries[index].path, $event)"
          @dblclick="emit('activate', entries[index].path)"
          @contextmenu.prevent="emit('contextmenu', entries[index].path, $event)"
        >
          <StatusGlyph
            :view="statusView(entries[index].status)"
            :code="entries[index].status"
          />
          <span class="changeset-path" :title="labelOf(entries[index])">{{ labelOf(entries[index]) }}</span>
          <span v-if="entries[index].binary" class="changeset-binary">{{ t("commit.binary") }}</span>
          <span v-else class="changeset-counts">
            <span class="count-add">+{{ entries[index].additions }}</span>
            <span class="count-del">−{{ entries[index].deletions }}</span>
          </span>
        </div>
      </template>
    </VirtualList>
  </div>
</template>

<style scoped>
.changeset {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}

.changeset-summary {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-1) var(--md-space-3);
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  border-bottom: 1px solid var(--md-border-inactive);
}

.summary-add {
  color: var(--md-status-added-fg);
}

.summary-del {
  color: var(--md-status-deleted-fg);
}

.changeset-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  height: 24px;
  padding: 0 var(--md-space-2);
  font-size: var(--md-text-sm);
}

.changeset-row:hover {
  background: var(--md-bg-hover);
}

.changeset-row.selected {
  background: var(--md-bg-selected);
}

.changeset-path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.changeset-binary {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.changeset-counts {
  display: inline-flex;
  gap: var(--md-space-1);
  font-size: var(--md-text-xs);
  font-variant-numeric: tabular-nums;
}

.count-add {
  color: var(--md-status-added-fg);
}

.count-del {
  color: var(--md-status-deleted-fg);
}
</style>
