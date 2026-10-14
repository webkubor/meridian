<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

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

const { t } = useI18n();

/**
 * Tab names come from `entry.labelKey`, not `entry.label`: the route table owns
 * one id → catalogue-key map so the command switch and this tab bar cannot
 * drift into two names for the same tab.
 */

/** Shared with `RepoSwitcher`: one word for "a git operation is running". */
const stateLabel = computed<string | null>(() => {
  const state = props.summary?.state;
  if (!state || state === "clean") return null;
  if (state === "merge") return t("screens.operation.state.merging");
  if (state === "rebase" || state === "rebaseMerge" || state === "rebaseInteractive")
    return t("screens.operation.state.rebasing");
  if (state === "cherryPick" || state === "cherryPickSequence")
    return t("screens.operation.state.cherryPicking");
  if (state === "revert" || state === "revertSequence") return t("screens.operation.state.reverting");
  if (state === "bisect") return t("screens.operation.state.bisecting");
  if (state === "applyMailbox" || state === "applyMailboxOrRebase")
    return t("screens.operation.state.applyingPatches");
  return t("screens.operation.state.inProgress");
});
</script>

<template>
  <div class="repo-screen">
    <nav v-if="summary" class="workspace-tabs" role="tablist" :aria-label="$t('screens.repository.tabsLabel')">
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
        {{ $t(entry.labelKey) }}
      </button>
    </nav>

    <div v-if="stateLabel" class="integration-banner" role="status">
      <Icon name="exclamationmark.triangle" :size="13" />
      <span>{{ $t('screens.operation.bannerHint', { state: stateLabel }) }}</span>
      <span class="banner-spacer" />
      <button type="button" class="banner-verb" @click="emit('continueOperation')">
        {{ $t('screens.operation.continue') }}
      </button>
      <button type="button" class="banner-verb danger" @click="emit('abortOperation')">
        {{ $t('screens.operation.abort') }}
      </button>
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
        :title="$t('screens.repository.noRepositoryOpen')"
        :caption="$t('screens.repository.noRepositoryCaption')"
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
