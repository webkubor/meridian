<script setup lang="ts">
import { computed } from "vue";

import type {
  Changeset,
  CommitDetail,
  DiffDocument,
  DiffOptions,
  DiffViewMode,
  HunkAction,
  HunkRef,
  LoadState,
  RefChip,
  SignatureState,
} from "../contracts";
import DiffViewer from "../diff/DiffViewer.vue";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import MasterDetail from "../../layout/MasterDetail.vue";
import ChangesetList from "./ChangesetList.vue";

/**
 * `<CommitDetailPane>` — the commit-detail pane (docs/04 §S8): the commit
 * header (summary, hash with copy, ref chips, signature badge, author line),
 * the full message, then the changeset beside the selected file's diff.
 */
const props = withDefaults(
  defineProps<{
    state: LoadState<CommitDetail>;
    changeset: LoadState<Changeset>;
    diff: LoadState<DiffDocument> | null;
    diffMode: DiffViewMode;
    diffOptions: DiffOptions;
    selectedPath: string | null;
    detailWidth: number;
    /** The commit being compared against (parent when a merge is selected). */
    parentLabel?: string | null;
  }>(),
  { parentLabel: null },
);

const emit = defineEmits<{
  copyHash: [hash: string];
  copySummary: [text: string];
  selectFile: [path: string, event: MouseEvent];
  fileContextmenu: [path: string, event: MouseEvent];
  openBlame: [path: string];
  openInEditor: [path: string];
  browseTree: [oid: string];
  selectParent: [oid: string];
  "update:detailWidth": [value: number];
  "update:diffMode": [mode: DiffViewMode];
  "update:diffOptions": [options: DiffOptions];
  hunkAction: [payload: { action: HunkAction; hunk: HunkRef; path: string }];
  diffRetry: [];
  diffShowAnyway: [];
}>();

const commit = computed(() => (props.state.status === "data" ? props.state.data.commit : null));
const detail = computed(() => (props.state.status === "data" ? props.state.data : null));

const refChips = computed<RefChip[]>(() => detail.value?.refs ?? []);

const signature = computed(() => detail.value?.signature ?? null);

const SIGNATURE: Record<SignatureState, { icon: string; tone: string; label: string }> = {
  unsigned: { icon: "questionmark.circle", tone: "muted", label: "Not signed" },
  present: { icon: "signature", tone: "muted", label: "Signed (not verified)" },
  valid: { icon: "checkmark.circle.fill", tone: "positive", label: "Signature verified" },
  untrusted: { icon: "exclamationmark.triangle", tone: "warning", label: "Signed by an untrusted key" },
  invalid: { icon: "exclamationmark.circle", tone: "danger", label: "Signature invalid" },
  expired: { icon: "clock", tone: "warning", label: "Signature expired" },
};

function chipIcon(kind: RefChip["kind"]): string {
  if (kind === "head") return "arrow.triangle.branch";
  if (kind === "tag") return "tag";
  if (kind === "remoteBranch") return "cloud";
  return "arrow.triangle.branch";
}

const date = computed(() => {
  const time = commit.value?.author.time;
  if (!time) return null;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(time * 1000));
});
</script>

<template>
  <div class="commit-detail">
    <EmptyState
      v-if="state.status === 'loading'"
      icon="clock"
      title="Loading commit…"
      compact
    />
    <EmptyState
      v-else-if="state.status === 'empty'"
      icon="empty.noCommitSelected"
      title="No commit selected"
      caption="Pick a commit from the history to see its details."
    />
    <EmptyState
      v-else-if="state.status === 'error'"
      icon="exclamationmark.circle"
      title="Could not load the commit"
      :caption="state.error.message"
      action-label="Try again"
      compact
      @action="emit('diffRetry')"
    />

    <template v-else-if="detail">
      <header class="commit-header">
        <div class="commit-summary-row">
          <h2 class="commit-summary" :title="commit?.message">{{ commit?.summary }}</h2>
          <span class="commit-spacer" />
          <button type="button" class="commit-verb" title="Copy summary" @click="emit('copySummary', commit?.summary ?? '')">
            <Icon name="doc.on.doc" :size="13" />
          </button>
        </div>

        <div class="commit-meta">
          <button
            type="button"
            class="commit-hash mono"
            title="Copy the full hash"
            @click="emit('copyHash', commit?.oid ?? '')"
          >
            {{ commit?.shortOid }}
            <Icon name="doc.on.doc" :size="11" />
          </button>

          <span v-for="chip in refChips" :key="`${chip.kind}:${chip.name}`" class="ref-chip" :class="`kind-${chip.kind}`">
            <Icon :name="chipIcon(chip.kind)" :size="11" />
            {{ chip.name }}
          </span>

          <span class="commit-spacer" />

          <span v-if="signature" class="sig-badge" :class="`tone-${SIGNATURE[signature.state].tone}`">
            <Icon :name="SIGNATURE[signature.state].icon" :size="12" />
            {{ SIGNATURE[signature.state].label }}
            <template v-if="signature.signer"> · {{ signature.signer }}</template>
          </span>
        </div>

        <div class="commit-author">
          <Icon name="person.crop.circle" :size="13" />
          <span class="author-name">{{ commit?.author.name }}</span>
          <span class="author-email">&lt;{{ commit?.author.email }}&gt;</span>
          <span v-if="date" class="author-date">{{ date }}</span>
          <span class="commit-spacer" />
          <button
            v-if="commit?.parents.length"
            type="button"
            class="commit-verb"
            :title="`Browse the tree at ${parentLabel ?? 'this commit'}`"
            @click="emit('browseTree', commit.oid)"
          >
            <Icon name="folder" :size="13" />
            Browse files
          </button>
        </div>

        <div v-if="commit?.parents.length" class="commit-parents">
          <span class="parents-label">{{ commit.parents.length > 1 ? "Parents" : "Parent" }}</span>
          <button
            v-for="parent in commit.parents"
            :key="parent"
            type="button"
            class="parent-link mono"
            @click="emit('selectParent', parent)"
          >
            {{ parent.slice(0, 7) }}
          </button>
        </div>
      </header>

      <pre class="commit-message">{{ commit?.message }}</pre>

      <MasterDetail
        :detail-width="detailWidth"
        :min="320"
        :max="900"
        @update:detail-width="emit('update:detailWidth', $event)"
      >
        <template #master>
          <ChangesetList
            v-if="changeset.status === 'data'"
            :changeset="changeset.data"
            :selected-path="selectedPath"
            @select="(path, event) => emit('selectFile', path, event)"
            @contextmenu="(path, event) => emit('fileContextmenu', path, event)"
          />
          <EmptyState
            v-else-if="changeset.status === 'loading'"
            icon="hourglass"
            title="Loading files…"
            compact
          />
          <EmptyState
            v-else-if="changeset.status === 'error'"
            icon="exclamationmark.circle"
            :title="changeset.error.message"
            compact
          />
          <EmptyState v-else icon="empty.multiSelection" title="No changed files" compact />
        </template>

        <template #detail>
          <DiffViewer
            v-if="diff && selectedPath"
            :state="diff"
            :mode="diffMode"
            :options="diffOptions"
            :path="selectedPath"
            :hunk-actions="false"
            @update:mode="emit('update:diffMode', $event)"
            @update:options="emit('update:diffOptions', $event)"
            @hunk-action="emit('hunkAction', $event)"
            @retry="emit('diffRetry')"
            @show-anyway="emit('diffShowAnyway')"
          />
          <EmptyState
            v-else-if="selectedPath"
            icon="empty.noFileSelected"
            title="Commit diffs are not available yet"
            caption="The command that loads one file's diff at a commit is not in this build; the changeset entry above is shown instead."
          />
          <EmptyState
            v-else
            icon="empty.noFileSelected"
            title="No file selected"
            caption="Choose a file to see its diff."
          />
        </template>
      </MasterDetail>
    </template>
  </div>
</template>

<style scoped>
.commit-detail {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

.commit-header {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
  padding: var(--md-space-2) var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
  background: var(--md-bg-header);
}

.commit-summary-row {
  display: flex;
  align-items: baseline;
  gap: var(--md-space-2);
}

.commit-summary {
  margin: 0;
  font-size: var(--md-text-md);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.commit-spacer {
  flex: 1;
}

.commit-meta,
.commit-author {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  flex-wrap: wrap;
}

.commit-hash {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 1px solid var(--md-border);
  border-radius: var(--md-radius-sm);
  background: var(--md-bg-sunken);
  padding: 0 var(--md-space-1);
  font-size: var(--md-text-xs);
  color: var(--md-fg);
}

.ref-chip {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  border-radius: var(--md-radius-sm);
  padding: 0 var(--md-space-1);
  background: var(--md-badge-bg);
  color: var(--md-badge-fg);
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ref-chip.kind-head {
  background: var(--md-bg-selected);
  color: var(--md-accent);
  font-weight: 600;
}

.ref-chip.kind-tag {
  color: var(--md-warning);
}

.sig-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.sig-badge.tone-positive {
  color: var(--md-success);
}

.sig-badge.tone-warning {
  color: var(--md-warning);
}

.sig-badge.tone-danger {
  color: var(--md-danger);
}

.sig-badge.tone-muted {
  color: var(--md-fg-subtle);
}

.author-email {
  color: var(--md-fg-subtle);
}

.commit-verb {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  font-size: var(--md-text-xs);
  padding: 1px var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.commit-verb:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.commit-parents {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.parent-link {
  border: none;
  background: transparent;
  color: var(--md-accent);
  padding: 0;
  font-size: var(--md-text-xs);
}

.parent-link:hover {
  text-decoration: underline;
  background: transparent;
}

.commit-message {
  margin: 0;
  padding: var(--md-space-2) var(--md-space-3);
  max-height: 140px;
  overflow-y: auto;
  font-family: var(--md-font-ui);
  font-size: var(--md-text-sm);
  line-height: var(--md-leading-normal);
  white-space: pre-wrap;
  color: var(--md-fg);
  border-bottom: 1px solid var(--md-border);
}
</style>
