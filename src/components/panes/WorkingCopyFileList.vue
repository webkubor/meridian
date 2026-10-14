<script setup lang="ts">
import { computed } from "vue";

import type { EntryStatus } from "../../bridges/bindings";
import Icon from "../common/Icon.vue";
import StatusGlyph from "../common/StatusGlyph.vue";
import VirtualList from "../common/VirtualList.vue";

/**
 * `<WorkingCopyFileList>` — one section of the working-copy list (docs/04 §S6):
 * a header with the section count and a bulk action, then the entries. Files
 * keep their two-axis status (staged ↔ worktree) via `<StatusGlyph>`.
 *
 * The rows carry per-row actions on hover; row *selection* and the bulk verbs
 * are reported upward. The list is virtualised because a build tree can hold
 * tens of thousands of untracked files.
 */
const props = withDefaults(
  defineProps<{
    title: string;
    entries: EntryStatus[];
    selectedPath?: string | null;
    /** Bulk verb label + which way it moves entries. */
    bulkAction?: "stage" | "unstage" | null;
    /** Section-level empty text (e.g. "No staged changes"). */
    emptyCaption?: string;
    /** Shows the discard verb (destructive; two-step handled by the shell). */
    allowDiscard?: boolean;
    /** Multi-select set, when the shell supports it. */
    selectedPaths?: string[];
  }>(),
  {
    selectedPath: null,
    bulkAction: null,
    emptyCaption: "No files",
    allowDiscard: true,
    selectedPaths: () => [],
  },
);

const emit = defineEmits<{
  select: [path: string, event: MouseEvent];
  toggle: [path: string];
  stage: [path: string];
  unstage: [path: string];
  discard: [path: string];
  bulk: [];
  contextmenu: [path: string, event: MouseEvent];
}>();

/** Directory part of the path, for the muted prefix column. */
function dirOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? "" : path.slice(0, index + 1);
}

function nameOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? path : path.slice(index + 1);
}

const selectedSet = computed(() => new Set(props.selectedPaths));
</script>

<template>
  <section class="file-section">
    <header class="section-head">
      <span class="section-title">{{ title }}</span>
      <span class="section-count">{{ entries.length }}</span>
      <span class="section-spacer" />
      <button
        v-if="bulkAction && entries.length > 0"
        type="button"
        class="section-bulk"
        :title="bulkAction === 'stage' ? `Stage all ${title.toLowerCase()}` : `Unstage all ${title.toLowerCase()}`"
        @click="emit('bulk')"
      >
        <Icon :name="bulkAction === 'stage' ? 'plus.circle' : 'minus.circle'" :size="12" />
        {{ bulkAction === "stage" ? "Stage all" : "Unstage all" }}
      </button>
    </header>

    <p v-if="entries.length === 0" class="section-empty">{{ emptyCaption }}</p>

    <VirtualList
      v-else
      :count="entries.length"
      :row-height="24"
      class="file-list"
      @end-reached="() => {}"
    >
      <template #default="{ index }">
        <div
          class="file-row"
          :class="{ selected: entries[index].path === selectedPath }"
          role="option"
          :aria-selected="entries[index].path === selectedPath"
          @click="emit('select', entries[index].path, $event)"
          @contextmenu.prevent="emit('contextmenu', entries[index].path, $event)"
        >
          <input
            v-if="selectedPaths"
            type="checkbox"
            class="file-check"
            :checked="selectedSet.has(entries[index].path)"
            :aria-label="`Select ${entries[index].path}`"
            @click.stop
            @change="emit('toggle', entries[index].path)"
          />
          <StatusGlyph :view="entries[index].view" :code="entries[index].code" :staged="entries[index].staged" />
          <span class="file-path">
            <span class="file-dir">{{ dirOf(entries[index].path) }}</span>
            <span class="file-name">{{ nameOf(entries[index].path) }}</span>
          </span>

          <span class="file-actions">
            <button
              v-if="entries[index].staged"
              type="button"
              class="file-action"
              title="Unstage"
              @click.stop="emit('unstage', entries[index].path)"
            >
              <Icon name="minus.circle" :size="13" />
            </button>
            <button
              v-else
              type="button"
              class="file-action"
              title="Stage"
              @click.stop="emit('stage', entries[index].path)"
            >
              <Icon name="plus.circle" :size="13" />
            </button>
            <button
              v-if="allowDiscard && !entries[index].staged"
              type="button"
              class="file-action danger"
              title="Discard changes"
              @click.stop="emit('discard', entries[index].path)"
            >
              <Icon name="arrow.uturn.backward" :size="12" />
            </button>
            <button
              v-if="entries[index].assumeUnchanged"
              type="button"
              class="file-action muted"
              title="Assume unchanged"
              disabled
            >
              <Icon name="eye.slash" :size="12" />
            </button>
          </span>
        </div>
      </template>
    </VirtualList>
  </section>
</template>

<style scoped>
.file-section {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.section-head {
  position: sticky;
  top: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  min-height: 22px;
  padding: 0 var(--md-space-2);
  background: var(--md-bg-header-translucent);
  backdrop-filter: saturate(1.6) blur(12px);
  border-bottom: 1px solid var(--md-border-inactive);
}

.section-title {
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.section-count {
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  font-variant-numeric: tabular-nums;
}

.section-spacer {
  flex: 1;
}

.section-bulk {
  border: none;
  background: transparent;
  color: var(--md-accent);
  font-size: var(--md-text-xs);
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 1px var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.section-bulk:hover {
  background: var(--md-bg-hover);
}

.section-empty {
  margin: 0;
  padding: var(--md-space-1) var(--md-space-3) var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
}

.file-list {
  max-height: 40vh;
}

.file-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  height: 24px;
  padding: 0 var(--md-space-2);
  font-size: var(--md-text-sm);
}

.file-row:hover {
  background: var(--md-bg-hover);
}

.file-row.selected {
  background: var(--md-bg-selected);
}

.file-check {
  flex: none;
  width: 13px;
  height: 13px;
  padding: 0;
  accent-color: var(--md-accent);
}

.file-path {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: baseline;
  overflow: hidden;
  white-space: nowrap;
}

.file-dir {
  color: var(--md-fg-subtle);
  overflow: hidden;
  text-overflow: ellipsis;
}

.file-name {
  color: var(--md-fg);
}

.file-actions {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 2px;
  opacity: 0;
}

.file-row:hover .file-actions,
.file-row.selected .file-actions,
.file-actions:focus-within {
  opacity: 1;
}

.file-action {
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  padding: 2px;
  border-radius: var(--md-radius-sm);
  display: inline-flex;
}

.file-action:hover:not(:disabled) {
  background: var(--md-bg-active);
  color: var(--md-fg);
}

.file-action.danger:hover:not(:disabled) {
  color: var(--md-danger);
}

.file-action.muted {
  color: var(--md-fg-disabled);
}
</style>
