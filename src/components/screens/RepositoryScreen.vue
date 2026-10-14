<script setup lang="ts">
import { computed } from "vue";

import type { RepoSummaryModel } from "../contracts";
import { WORKSPACE_TABS, type WorkspaceTab } from "../../router/routes";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";

/**
 * `<RepositoryScreen>` — the main window (docs/04 §S3/S6/S8/S9): the tab bar
 * (Working copy / History / Branches / Compare) under the window chrome, the
 * integration banner while an operation is in progress, and the active tab's
 * pane. The panes arrive through named slots so this screen stays a pure host.
 */
const props = defineProps<{
  summary: RepoSummaryModel | null;
  tab: WorkspaceTab;
}>();

const emit = defineEmits<{
  "update:tab": [tab: WorkspaceTab];
  /** Abort/continue the in-progress operation from the banner. */
  continueOperation: [];
  abortOperation: [];
}>();

const stateLabel = computed<string | null>(() => {
  const state = props.summary?.state;
  if (!state || state === "clean") return null;
  if (state === "merge") return "Merging";
  if (state === "rebase" || state === "rebaseMerge" || state === "rebaseInteractive") return "Rebasing";
  if (state === "cherryPick" || state === "cherryPickSequence") return "Cherry-picking";
  if (state === "revert" || state === "revertSequence") return "Reverting";
  if (state === "bisect") return "Bisecting";
  if (state === "applyMailbox" || state === "applyMailboxOrRebase") return "Applying patches";
  return "Operation in progress";
});
</script>

<template>
  <div class="repo-screen">
    <nav v-if="summary" class="workspace-tabs" role="tablist" aria-label="Repository view">
      <button
        v-for="entry in WORKSPACE_TABS"
        :key="entry.id"
        type="button"
        role="tab"
        :aria-selected="entry.id === tab"
        class="workspace-tab"
        :class="{ active: entry.id === tab }"
        @click="emit('update:tab', entry.id)"
      >
        <Icon :name="entry.icon" :size="13" />
        {{ entry.label }}
      </button>
    </nav>

    <div v-if="stateLabel" class="integration-banner" role="status">
      <Icon name="exclamationmark.triangle" :size="13" />
      <span>{{ stateLabel }}. Finish or abort the operation to continue.</span>
      <span class="banner-spacer" />
      <button type="button" class="banner-verb" @click="emit('continueOperation')">Continue</button>
      <button type="button" class="banner-verb danger" @click="emit('abortOperation')">Abort</button>
    </div>

    <div class="workspace-body">
      <template v-if="summary">
        <slot v-if="tab === 'working-copy'" name="working-copy" />
        <slot v-else-if="tab === 'history'" name="history" />
        <slot v-else-if="tab === 'branches'" name="branches" />
        <slot v-else name="compare" />
      </template>

      <EmptyState
        v-else
        icon="empty.noRepositorySelected"
        title="No repository open"
        caption="Open a repository from the repository list."
      />
    </div>
  </div>
</template>

<style scoped>
.repo-screen {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

.workspace-tabs {
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  height: var(--md-tab-height);
  flex: none;
  padding: 0 var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
  background: var(--md-bg-header);
}

.workspace-tab {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-1);
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  font-size: var(--md-text-sm);
  padding: var(--md-space-1) var(--md-space-2);
  border-radius: var(--md-radius-sm);
}

.workspace-tab:hover {
  color: var(--md-fg);
  background: var(--md-bg-hover);
}

.workspace-tab.active {
  color: var(--md-fg);
  background: var(--md-bg-selected);
  font-weight: 600;
}

.integration-banner {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  flex: none;
  padding: var(--md-space-1) var(--md-space-3);
  background: var(--md-bg-warning-soft);
  color: var(--md-fg);
  font-size: var(--md-text-sm);
  border-bottom: 1px solid var(--md-border);
}

.integration-banner :deep(svg) {
  color: var(--md-warning);
}

.banner-spacer {
  flex: 1;
}

.banner-verb {
  border: none;
  background: transparent;
  color: var(--md-accent);
  font-size: var(--md-text-sm);
  font-weight: 600;
  padding: 0 var(--md-space-1);
}

.banner-verb.danger {
  color: var(--md-danger);
}

.banner-verb:hover {
  background: transparent;
  text-decoration: underline;
}

.workspace-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
</style>
