<script setup lang="ts">
import { computed } from "vue";

import type { EntryStatus, StatusSummary } from "../../bridges/bindings";
import type {
  DiffDocument,
  DiffOptions,
  DiffViewMode,
  HunkAction,
  HunkRef,
  LoadState,
  MergeStatus,
  SplitActionOption,
} from "../contracts";
import Icon from "../common/Icon.vue";
import ListShell from "../common/ListShell.vue";
import MasterDetail from "../../layout/MasterDetail.vue";
import CommitComposer from "./CommitComposer.vue";
import WorkingCopyFileDetail from "./WorkingCopyFileDetail.vue";
import WorkingCopyFileList from "./WorkingCopyFileList.vue";

/**
 * `<WorkingCopyPane>` — the working-copy tab (docs/04 §S6): the merge banner,
 * the grouped file sections (Conflicts / Staged / Modified / Untracked), the
 * commit composer, and the file diff beside them. All state arrives as props;
 * the pane groups entries and forwards intent.
 */
const props = withDefaults(
  defineProps<{
    state: LoadState<EntryStatus[]>;
    summary: StatusSummary | null;
    selectedPath: string | null;
    merge: MergeStatus | null;
    conflictCount: number;

    /* detail column */
    detailWidth: number;
    selectedEntry: EntryStatus | null;
    diff: LoadState<DiffDocument> | null;
    diffMode: DiffViewMode;
    diffOptions: DiffOptions;

    /* composer */
    subject: string;
    body: string;
    amend: boolean;
    signOff: boolean;
    canAmend: boolean;
    hooksEnabled: boolean;
    hookRunning: boolean;
    hookPhase: string | null;
    commitBusy: boolean;
    commitOptions: SplitActionOption[];

    /** Show only conflicted files (the merge banner toggle). */
    conflictedOnly?: boolean;
  }>(),
  { summary: null, merge: null, conflictedOnly: false },
);

const emit = defineEmits<{
  refresh: [];
  selectFile: [path: string, event: MouseEvent];
  contextmenu: [path: string, event: MouseEvent];
  stage: [path: string];
  unstage: [path: string];
  discard: [path: string];
  bulkStage: [paths: string[]];
  bulkUnstage: [paths: string[]];
  "update:conflictedOnly": [value: boolean];
  "update:detailWidth": [value: number];
  "update:diffMode": [mode: DiffViewMode];
  "update:diffOptions": [options: DiffOptions];
  hunkAction: [payload: { action: HunkAction; hunk: HunkRef; path: string }];
  diffRetry: [];
  diffShowAnyway: [];
  openBlame: [path: string];
  openInEditor: [path: string];
  resolveConflict: [path: string];
  resolveMerge: [];
  abortMerge: [];

  /* composer */
  "update:subject": [value: string];
  "update:body": [value: string];
  "update:amend": [value: boolean];
  "update:signOff": [value: boolean];
  commit: [];
  commitChoose: [optionId: string];
}>();

const entries = computed(() => (props.state.status === "data" ? props.state.data : []));

const conflicted = computed(() => entries.value.filter((entry) => entry.code === "unmerged"));
const staged = computed(() => entries.value.filter((entry) => entry.staged && entry.code !== "unmerged"));
const modified = computed(
  () => entries.value.filter((entry) => !entry.staged && entry.code !== "untracked" && entry.code !== "unmerged"),
);
const untracked = computed(() => entries.value.filter((entry) => entry.code === "untracked"));

const visibleSections = computed(() => {
  if (props.conflictedOnly) {
    return [{ id: "conflicted", title: "Conflicts", entries: conflicted.value }];
  }
  return [
    { id: "conflicted", title: "Conflicts", entries: conflicted.value },
    { id: "staged", title: "Staged", entries: staged.value },
    { id: "modified", title: "Modified", entries: modified.value },
    { id: "untracked", title: "Untracked", entries: untracked.value },
  ];
});

const bulkActionFor = (id: string): "stage" | "unstage" | null => {
  if (id === "staged") return "unstage";
  if (id === "conflicted") return null;
  return "stage";
};

function onBulk(id: string, list: EntryStatus[]): void {
  const paths = list.map((entry) => entry.path);
  if (id === "staged") emit("bulkUnstage", paths);
  else emit("bulkStage", paths);
}
</script>

<template>
  <ListShell
    :status="state.status"
    empty-icon="empty.unchangedFile"
    empty-title="No local changes"
    empty-caption="Your working copy matches the last commit."
    :error-message="state.status === 'error' ? state.error.message : null"
    :error-detail="state.status === 'error' ? state.error.detail : null"
    loading-caption="Reading the working copy…"
    @retry="emit('refresh')"
  >
    <template #toolbar>
      <div class="wc-toolbar">
        <button
          type="button"
          class="wc-refresh"
          title="Refresh the working copy"
          @click="emit('refresh')"
        >
          <Icon name="arrow.clockwise" :size="13" />
        </button>
        <span v-if="summary" class="wc-summary">
          {{ summary.staged }} staged · {{ summary.unstaged }} modified · {{ summary.untracked }} untracked
        </span>
        <span class="wc-spacer" />
        <label v-if="conflictCount > 0" class="wc-conflict-toggle">
          <input
            type="checkbox"
            :checked="conflictedOnly"
            @change="emit('update:conflictedOnly', ($event.target as HTMLInputElement).checked)"
          />
          Conflicts only ({{ conflictCount }})
        </label>
      </div>
    </template>

    <div v-if="merge" class="merge-banner" role="region" aria-label="Merge status">
      <Icon name="arrow.triangle.merge" :size="14" />
      <span class="merge-text">
        <template v-if="merge.state === 'conflict'">
          Merging <strong>{{ merge.incoming?.label ?? "incoming changes" }}</strong> —
          {{ merge.conflicts.total - merge.conflicts.resolved }} of {{ merge.conflicts.total }} conflicts left.
        </template>
        <template v-else-if="merge.state === 'merged'">
          All conflicts resolved. Commit to finish the merge.
        </template>
        <template v-else>
          <strong>{{ merge.incoming?.label ?? "Incoming changes" }}</strong>
          ({{ merge.incoming?.commits ?? 0 }} commits) can be merged cleanly.
        </template>
      </span>
      <span class="merge-spacer" />
      <button v-if="merge.state === 'merged'" type="button" class="btn primary" @click="emit('resolveMerge')">
        Commit merge
      </button>
      <button type="button" class="btn" @click="emit('abortMerge')">Abort</button>
    </div>

    <MasterDetail
      :detail-width="detailWidth"
      :min="320"
      :max="900"
      @update:detail-width="emit('update:detailWidth', $event)"
    >
      <template #master>
        <div class="wc-list">
          <WorkingCopyFileList
            v-for="section in visibleSections"
            :key="section.id"
            :title="section.title"
            :entries="section.entries"
            :selected-path="selectedPath"
            :bulk-action="bulkActionFor(section.id)"
            :allow-discard="section.id !== 'conflicted' && section.id !== 'staged'"
            :empty-caption="section.id === 'conflicted' ? 'No conflicts' : `No ${section.title.toLowerCase()} changes`"
            @select="(path, event) => emit('selectFile', path, event)"
            @contextmenu="(path, event) => emit('contextmenu', path, event)"
            @stage="emit('stage', $event)"
            @unstage="emit('unstage', $event)"
            @discard="emit('discard', $event)"
            @bulk="onBulk(section.id, section.entries)"
          />
        </div>

        <CommitComposer
          :subject="subject"
          :body="body"
          :summary="summary ?? { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 }"
          :amend="amend"
          :sign-off="signOff"
          :can-amend="canAmend"
          :hooks-enabled="hooksEnabled"
          :hook-running="hookRunning"
          :hook-phase="hookPhase"
          :busy="commitBusy"
          :options="commitOptions"
          @update:subject="emit('update:subject', $event)"
          @update:body="emit('update:body', $event)"
          @update:amend="emit('update:amend', $event)"
          @update:sign-off="emit('update:signOff', $event)"
          @commit="emit('commit')"
          @choose="emit('commitChoose', $event)"
        />
      </template>

      <template #detail>
        <WorkingCopyFileDetail
          :entry="selectedEntry"
          :diff="diff"
          :mode="diffMode"
          :options="diffOptions"
          @update:mode="emit('update:diffMode', $event)"
          @update:options="emit('update:diffOptions', $event)"
          @hunk-action="emit('hunkAction', $event)"
          @stage="emit('stage', $event)"
          @unstage="emit('unstage', $event)"
          @discard="emit('discard', $event)"
          @open-blame="emit('openBlame', $event)"
          @open-in-editor="emit('openInEditor', $event)"
          @resolve-conflict="emit('resolveConflict', $event)"
          @retry="emit('diffRetry')"
          @show-anyway="emit('diffShowAnyway')"
        />
      </template>
    </MasterDetail>
  </ListShell>
</template>

<style scoped>
.wc-toolbar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: var(--md-toolbar-height);
  padding: 0 var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
}

.wc-refresh {
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  display: inline-flex;
  padding: var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.wc-refresh:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.wc-summary {
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}

.wc-spacer {
  flex: 1;
}

.wc-conflict-toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-1);
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}

.wc-conflict-toggle input {
  accent-color: var(--md-danger);
}

.merge-banner {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-2) var(--md-space-3);
  background: var(--md-bg-info-soft);
  color: var(--md-fg);
  border-bottom: 1px solid var(--md-border);
  font-size: var(--md-text-sm);
}

.merge-banner :deep(svg) {
  color: var(--md-info);
}

.merge-spacer {
  flex: 1;
}

.wc-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-bottom: var(--md-space-2);
}
</style>
