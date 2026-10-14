<script setup lang="ts">
import { computed, ref } from "vue";

import type { RepoSummaryModel } from "../components/contracts";
import Icon from "../components/common/Icon.vue";
import Popover from "../components/common/Popover.vue";

/**
 * `<RepoSwitcher>` — the sidebar's repository header (docs/04 §4, §S3): with
 * one repository open it is a plain header; clicking it lists the other recent
 * repositories and the entry point back to the repository list. A repository
 * mid-operation (merge/rebase/…) shows its action-state marker here.
 */
const props = withDefaults(
  defineProps<{
    current: RepoSummaryModel | null;
    recents: RepoSummaryModel[];
    /** Disables the switcher while a blocking operation runs. */
    locked?: boolean;
  }>(),
  { locked: false },
);

const emit = defineEmits<{
  switchRepo: [id: string];
  openRepositories: [];
}>();

const open = ref(false);

const others = computed(() =>
  props.recents.filter((repo) => repo.info.id !== props.current?.info.id),
);

const currentState = computed(() => props.current?.state ?? "clean");

const stateLabel = computed(() => {
  const state = currentState.value;
  if (state === "clean") return null;
  if (state === "merge") return "Merging";
  if (state === "rebase" || state === "rebaseMerge" || state === "rebaseInteractive") return "Rebasing";
  if (state === "cherryPick" || state === "cherryPickSequence") return "Cherry-picking";
  if (state === "revert" || state === "revertSequence") return "Reverting";
  if (state === "bisect") return "Bisecting";
  if (state === "applyMailbox" || state === "applyMailboxOrRebase") return "Applying patches";
  return "Operation in progress";
});

function choose(id: string): void {
  open.value = false;
  emit("switchRepo", id);
}

function browse(): void {
  open.value = false;
  emit("openRepositories");
}
</script>

<template>
  <Popover :open="open && !locked" align="start" label="Switch repository" @close="open = false">
    <template #trigger>
      <button
        type="button"
        class="switcher"
        :disabled="locked"
        :aria-expanded="open"
        aria-haspopup="menu"
        :title="current?.info.gitdir ?? 'No repository open'"
        @click="open = !open"
      >
        <Icon name="shippingbox" :size="15" class="switcher-icon" />
        <span class="switcher-names">
          <span class="switcher-name">{{ current?.headLabel ?? current?.info.gitdir ?? "Choose repository" }}</span>
          <span v-if="stateLabel" class="switcher-state">
            <Icon name="exclamationmark.triangle" :size="11" />
            {{ stateLabel }}
          </span>
        </span>
        <Icon name="chevron.up.chevron.down" :size="11" class="switcher-chevron" />
      </button>
    </template>

    <p class="popover-heading">Recent repositories</p>
    <p v-if="others.length === 0" class="popover-empty">No other repositories.</p>
    <button
      v-for="repo in others"
      :key="repo.info.id"
      type="button"
      class="popover-item"
      role="menuitem"
      :title="repo.info.gitdir"
      @click="choose(repo.info.id)"
    >
      <Icon name="shippingbox" :size="13" />
      <span class="popover-item-label">{{ repo.headLabel ?? repo.info.gitdir }}</span>
    </button>

    <div class="popover-divider" aria-hidden="true" />

    <button type="button" class="popover-item" role="menuitem" @click="browse">
      <Icon name="square.grid.2x2" :size="13" />
      <span class="popover-item-label">All repositories…</span>
    </button>
  </Popover>
</template>

<style scoped>
.switcher {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--md-fg);
  padding: var(--md-space-2) var(--md-space-2);
  border-radius: var(--md-radius-sm);
  text-align: left;
}

.switcher:hover:not(:disabled) {
  background: var(--md-bg-hover);
}

.switcher-icon {
  color: var(--md-accent);
}

.switcher-names {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.switcher-name {
  font-size: var(--md-text-sm);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.switcher-state {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--md-text-xs);
  color: var(--md-warning);
}

.switcher-chevron {
  color: var(--md-fg-subtle);
  flex: none;
}

.popover-heading {
  margin: 0;
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.popover-empty {
  margin: 0;
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
}

.popover-item {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  border-radius: var(--md-radius-sm);
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
  color: var(--md-fg);
}

.popover-item:hover {
  background: var(--md-bg-hover);
}

.popover-item-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.popover-divider {
  height: 1px;
  background: var(--md-border);
  margin: var(--md-space-1) var(--md-space-2);
}
</style>
