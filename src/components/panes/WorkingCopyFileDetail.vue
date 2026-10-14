<script setup lang="ts">
import type { EntryStatus } from "../../bridges/bindings";
import type {
  DiffDocument,
  DiffOptions,
  DiffViewMode,
  HunkAction,
  HunkRef,
  LoadState,
} from "../contracts";
import DiffViewer from "../diff/DiffViewer.vue";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import StatusGlyph from "../common/StatusGlyph.vue";

/**
 * `<WorkingCopyFileDetail>` — the detail column of the working-copy screen
 * (docs/04 §S6): the file's identity bar (status, path, actions) above the
 * diff, plus a conflicted-files notice that routes to the merge tooling. The
 * diff itself is `<DiffViewer>`; this component only supplies it the document
 * and the entry context.
 */
withDefaults(
  defineProps<{
    entry: EntryStatus | null;
    diff: LoadState<DiffDocument> | null;
    mode: DiffViewMode;
    options: DiffOptions;
  }>(),
  {},
);

const emit = defineEmits<{
  "update:mode": [mode: DiffViewMode];
  "update:options": [options: DiffOptions];
  hunkAction: [payload: { action: HunkAction; hunk: HunkRef; path: string }];
  stage: [path: string];
  unstage: [path: string];
  discard: [path: string];
  openBlame: [path: string];
  openInEditor: [path: string];
  resolveConflict: [path: string];
  retry: [];
  showAnyway: [];
}>();

function stagedFor(entry: EntryStatus | null): boolean {
  return entry?.staged ?? false;
}
</script>

<template>
  <div class="detail-pane">
    <template v-if="entry">
      <header class="file-bar">
        <StatusGlyph :view="entry.view" :code="entry.code" :staged="entry.staged" />
        <span class="file-bar-path" :title="entry.path">{{ entry.path }}</span>
        <span class="file-bar-spacer" />

        <button
          v-if="entry.code === 'unmerged'"
          type="button"
          class="bar-action"
          title="Open the merge tool for this file"
          @click="emit('resolveConflict', entry.path)"
        >
          <Icon name="arrow.triangle.merge" :size="13" />
          Resolve
        </button>
        <button type="button" class="bar-action" title="Blame this file" @click="emit('openBlame', entry.path)">
          <Icon name="signature" :size="13" />
          Blame
        </button>
        <button
          type="button"
          class="bar-action"
          title="Open in the default editor"
          @click="emit('openInEditor', entry.path)"
        >
          <Icon name="arrow.up.forward.square" :size="13" />
        </button>
        <span class="bar-divider" aria-hidden="true" />
        <button
          v-if="stagedFor(entry)"
          type="button"
          class="bar-action"
          @click="emit('unstage', entry.path)"
        >
          <Icon name="minus.circle" :size="13" />
          Unstage
        </button>
        <button v-else type="button" class="bar-action" @click="emit('stage', entry.path)">
          <Icon name="plus.circle" :size="13" />
          Stage
        </button>
        <button
          v-if="!stagedFor(entry) && entry.code !== 'unmerged'"
          type="button"
          class="bar-action danger"
          @click="emit('discard', entry.path)"
        >
          <Icon name="arrow.uturn.backward" :size="12" />
          Discard
        </button>
      </header>

      <div v-if="entry.code === 'unmerged'" class="conflict-note" role="status">
        <Icon name="exclamationmark.triangle" :size="13" />
        <span>This file has conflicts. Hunk staging is disabled until it is resolved.</span>
      </div>
    </template>

    <EmptyState
      v-else
      icon="empty.noFileSelected"
      title="No file selected"
      caption="Choose a file from the list to see its changes."
    />

    <DiffViewer
      v-if="entry && diff"
      :state="diff"
      :mode="mode"
      :options="options"
      :path="entry.path"
      :conflicted="entry.code === 'unmerged'"
      @update:mode="emit('update:mode', $event)"
      @update:options="emit('update:options', $event)"
      @hunk-action="emit('hunkAction', $event)"
      @retry="emit('retry')"
      @show-anyway="emit('showAnyway')"
    />
  </div>
</template>

<style scoped>
.detail-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

.file-bar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: 30px;
  padding: 0 var(--md-space-2);
  border-bottom: 1px solid var(--md-border);
  background: var(--md-bg-header);
}

.file-bar-path {
  font-size: var(--md-text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.file-bar-spacer {
  flex: 1;
}

.bar-divider {
  width: 1px;
  align-self: stretch;
  margin: var(--md-space-1) 0;
  background: var(--md-border-inactive);
}

.bar-action {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  font-size: var(--md-text-xs);
  padding: 2px var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.bar-action:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.bar-action.danger:hover {
  color: var(--md-danger);
}

.conflict-note {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-1) var(--md-space-3);
  background: var(--md-bg-warning-soft);
  color: var(--md-warning);
  font-size: var(--md-text-xs);
  border-bottom: 1px solid var(--md-border);
}
</style>
