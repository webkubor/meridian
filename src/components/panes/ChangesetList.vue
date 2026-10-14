<script setup lang="ts">
import { computed } from "vue";

import type { Changeset, ChangesetEntry } from "../contracts";
import type { UiStatusView } from "../../bridges/bindings";
import StatusGlyph from "../common/StatusGlyph.vue";
import VirtualList from "../common/VirtualList.vue";

/**
 * `<ChangesetList>` — the files of one commit / comparison side (docs/04 §S8).
 * A flat, virtualised list of `path  +A −D`, each row carrying its status
 * glyph. Renames show `old → new`; the row reports selection upward.
 */
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

const VIEW_BY_STATUS: Record<ChangesetEntry["status"], UiStatusView> = {
  added: { short: "A", text: "Added", tone: "positive" },
  copied: { short: "C", text: "Copied", tone: "positive" },
  deleted: { short: "D", text: "Deleted", tone: "danger" },
  modified: { short: "M", text: "Modified", tone: "info" },
  renamed: { short: "R", text: "Renamed", tone: "positive" },
  unmerged: { short: "U", text: "Unmerged", tone: "danger" },
  untracked: { short: "?", text: "Untracked", tone: "muted" },
  ignored: { short: "!", text: "Ignored", tone: "muted" },
};

const entries = computed(() => props.changeset.entries);

function labelOf(entry: ChangesetEntry): string {
  if (entry.oldPath && entry.oldPath !== entry.path) return `${entry.oldPath} → ${entry.path}`;
  return entry.path;
}
</script>

<template>
  <div class="changeset">
    <header v-if="showSummary" class="changeset-summary">
      <span class="summary-files">{{ changeset.stats.files }} files</span>
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
            :view="VIEW_BY_STATUS[entries[index].status]"
            :code="entries[index].status"
          />
          <span class="changeset-path" :title="labelOf(entries[index])">{{ labelOf(entries[index]) }}</span>
          <span v-if="entries[index].binary" class="changeset-binary">binary</span>
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
