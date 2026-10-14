<script setup lang="ts">
import { computed, ref } from "vue";

import type { RepoRoot } from "../../bridges/bindings";
import type { LoadState, RepoSummaryModel } from "../contracts";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import ListShell from "../common/ListShell.vue";
import RowBadge from "../common/RowBadge.vue";
import SearchField from "../common/SearchField.vue";

/**
 * `<RepositoryListScreen>` — the launch screen (docs/04 §S1/S2): the three
 * entry points (Open / Clone / Create), the recent repositories, and the
 * repositories discovered nearby. There is no repository context yet, so the
 * whole surface is offered as a single centred column.
 */
const props = withDefaults(
  defineProps<{
    recents: RepoSummaryModel[];
    recentsState: LoadState<RepoSummaryModel[]>;
    /**
     * Repositories found on disk but **not open** — hence `RepoRoot` rather
     * than `RepoSummaryModel`. Typing them as open repositories made the row
     * look clickable while the id it emitted was not one anything could open,
     * so the whole nearby list was a row of buttons that did nothing.
     */
    nearby: RepoRoot[];
    nearbyState: LoadState<RepoRoot[]>;
    pathInput: string;
    /** The recents filter — a separate model from `pathInput` (the Open/Create field). */
    filter?: string;
    busy?: boolean;
    /** Set when the last open/clone attempt failed. */
    errorMessage?: string | null;
    errorDetail?: string | null;
    cloneUrl?: string;
    showNearby?: boolean;
    /**
     * Reasons for entry points whose backend is not built: the Clone tab/button,
     * the folder picker and the nearby scan disable with these as the tooltip
     * instead of looking live and then apologising.
     */
    unavailable?: { browse?: string; clone?: string; nearby?: string } | null;
  }>(),
  {
    filter: "",
    busy: false,
    errorMessage: null,
    errorDetail: null,
    cloneUrl: "",
    showNearby: true,
    unavailable: null,
  },
);

const emit = defineEmits<{
  "update:pathInput": [value: string];
  "update:filter": [value: string];
  "update:cloneUrl": [value: string];
  open: [path: string];
  init: [path: string];
  browse: [];
  clone: [url: string];
  openRecent: [id: string];
  removeRecent: [id: string];
  revealRecent: [id: string];
  refreshNearby: [];
  openNearby: [path: string];
  openSettings: [];
}>();

const mode = ref<"open" | "clone" | "create">("open");

const filteredRecents = computed(() => {
  const needle = props.filter.trim().toLowerCase();
  if (needle === "") return props.recents;
  return props.recents.filter((repo) => repo.info.gitdir.toLowerCase().includes(needle));
});

function submit(): void {
  if (mode.value === "clone") {
    if (props.cloneUrl.trim() !== "") emit("clone", props.cloneUrl.trim());
  } else if (mode.value === "create") {
    emit("init", props.pathInput.trim());
  } else {
    emit("open", props.pathInput.trim());
  }
}

function labelOf(repo: RepoSummaryModel): string {
  const workdir = repo.info.workdir ?? repo.info.gitdir;
  const parts = workdir.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? workdir;
}
</script>

<template>
  <div class="launch">
    <div class="launch-column">
      <header class="launch-head">
        <h1 class="launch-title">{{ $t('app.name') }}</h1>
        <span class="launch-spacer" />
        <button type="button" class="launch-icon" :title="$t('screens.settings.title')" @click="emit('openSettings')">
          <Icon name="nav.settings" :size="15" />
        </button>
      </header>

      <div class="launch-modes" role="radiogroup" :aria-label="$t('screens.repositories.modeGroupLabel')">
        <button
          type="button"
          role="radio"
          :aria-checked="mode === 'open'"
          class="mode"
          :class="{ active: mode === 'open' }"
          @click="mode = 'open'"
        >
          <Icon name="folder" :size="14" />
          {{ $t('screens.repositories.open') }}
        </button>
        <button
          type="button"
          role="radio"
          :aria-checked="mode === 'clone'"
          class="mode"
          :class="{ active: mode === 'clone' }"
          :disabled="!!unavailable?.clone"
          :title="unavailable?.clone ?? undefined"
          @click="mode = 'clone'"
        >
          <Icon name="cloud" :size="14" />
          {{ $t('screens.repositories.clone') }}
        </button>
        <button
          type="button"
          role="radio"
          :aria-checked="mode === 'create'"
          class="mode"
          :class="{ active: mode === 'create' }"
          @click="mode = 'create'"
        >
          <Icon name="plus" :size="14" />
          {{ $t('screens.repositories.create') }}
        </button>
      </div>

      <form class="launch-form" @submit.prevent="submit">
        <input
          v-if="mode === 'clone'"
          class="launch-input"
          type="text"
          :value="cloneUrl"
          placeholder="https://github.com/owner/repository.git"
          :aria-label="$t('screens.repositories.urlLabel')"
          spellcheck="false"
          autocapitalize="off"
          autocorrect="off"
          @input="emit('update:cloneUrl', ($event.target as HTMLInputElement).value)"
        />
        <input
          v-else
          class="launch-input"
          type="text"
          :value="pathInput"
          :placeholder="
            mode === 'create'
              ? $t('screens.repositories.newFolderPlaceholder')
              : $t('screens.repositories.existingFolderPlaceholder')
          "
          :aria-label="
            mode === 'create'
              ? $t('screens.repositories.newFolderLabel')
              : $t('screens.repositories.openFolderLabel')
          "
          spellcheck="false"
          autocapitalize="off"
          autocorrect="off"
          @input="emit('update:pathInput', ($event.target as HTMLInputElement).value)"
        />
        <button
          v-if="mode !== 'clone'"
          type="button"
          class="launch-browse"
          :disabled="!!unavailable?.browse"
          :title="unavailable?.browse ?? $t('screens.repositories.chooseFolder')"
          @click="emit('browse')"
        >
          {{ $t('screens.choose') }}
        </button>
        <button
          type="submit"
          class="launch-go"
          :disabled="busy || (mode === 'clone' ? cloneUrl.trim() === '' || !!unavailable?.clone : pathInput.trim() === '')"
        >
          <template v-if="mode === 'open'">{{ $t('screens.repositories.open') }}</template>
          <template v-else-if="mode === 'clone'">{{ $t('screens.repositories.clone') }}</template>
          <template v-else>{{ $t('screens.repositories.create') }}</template>
        </button>
      </form>

      <p v-if="errorMessage" class="launch-error" role="alert">
        <Icon name="exclamationmark.circle" :size="13" />
        <span>{{ errorMessage }}</span>
        <span v-if="errorDetail" class="launch-error-detail mono">{{ errorDetail }}</span>
      </p>

      <div class="launch-recents">
        <div class="recents-head">
          <span class="recents-title">{{ $t('screens.repositories.recentsTitle') }}</span>
          <span class="launch-spacer" />
          <SearchField
            :model-value="filter"
            :label="$t('screens.repositories.filterLabel')"
            :placeholder="$t('screens.repositories.filterPlaceholder')"
            class="recents-filter"
            @update:model-value="emit('update:filter', $event)"
          />
        </div>

        <EmptyState
          v-if="recentsState.status === 'loading'"
          icon="hourglass"
          :title="$t('screens.repositories.loadingRecents')"
          compact
        />
        <EmptyState
          v-else-if="recentsState.status === 'error'"
          icon="exclamationmark.circle"
          :title="recentsState.error.message"
          compact
        />
        <EmptyState
          v-else-if="filteredRecents.length === 0"
          icon="empty.noRepositorySelected"
          :title="$t('screens.repositories.noRecents')"
          :caption="$t('screens.repositories.noRecentsCaption')"
        />
        <ul v-else class="recents-list">
          <li v-for="repo in filteredRecents" :key="repo.info.id">
            <button
              type="button"
              class="recent-row"
              @click="emit('openRecent', repo.info.id)"
            >
              <Icon name="shippingbox" :size="15" class="recent-icon" />
              <span class="recent-body">
                <span class="recent-name">{{ labelOf(repo) }}</span>
                <span class="recent-path mono">{{ repo.info.gitdir }}</span>
              </span>
              <RowBadge v-if="repo.headLabel" :label="repo.headLabel" icon="arrow.triangle.branch" tone="neutral" />
              <RowBadge v-if="repo.state !== 'clean'" icon="exclamationmark.triangle" tone="warning" />
            </button>
            <span class="recent-actions">
              <button type="button" class="recent-action" :title="$t('screens.repositories.reveal')" @click="emit('revealRecent', repo.info.id)">
                <Icon name="action.reveal" :size="13" />
              </button>
              <button type="button" class="recent-action" :title="$t('screens.repositories.removeRecent')" @click="emit('removeRecent', repo.info.id)">
                <Icon name="xmark" :size="13" />
              </button>
            </span>
          </li>
        </ul>
      </div>

      <section v-if="showNearby" class="launch-nearby">
        <div class="nearby-head">
          <span class="recents-title">{{ $t('screens.repositories.nearbyTitle') }}</span>
          <span class="launch-spacer" />
          <button
            type="button"
            class="nearby-refresh"
            :disabled="!!unavailable?.nearby"
            :title="unavailable?.nearby ?? $t('screens.repositories.scanAgain')"
            @click="emit('refreshNearby')"
          >
            <Icon name="arrow.clockwise" :size="12" />
            {{ $t('screens.repositories.scanAgain') }}
          </button>
        </div>
        <ListShell
          :status="nearbyState.status"
          empty-icon="empty.dropFolder"
          :empty-title="$t('screens.repositories.nothingNearby')"
          :empty-caption="$t('screens.repositories.nothingNearbyCaption')"
          :loading-caption="$t('screens.repositories.scanning')"
          :error-message="nearbyState.status === 'error' ? nearbyState.error.message : null"
          @retry="emit('refreshNearby')"
        >
          <ul class="nearby-list">
            <li v-for="repo in nearby" :key="repo.gitdir">
              <button
                type="button"
                class="nearby-row"
                :title="repo.path"
                @click="emit('openNearby', repo.path)"
              >
                <Icon name="folder" :size="13" />
                <span class="nearby-name">{{ repo.name }}</span>
                <span class="mono nearby-path">{{ repo.gitdir }}</span>
              </button>
            </li>
          </ul>
        </ListShell>
      </section>
    </div>
  </div>
</template>

<style scoped>
.launch {
  display: flex;
  justify-content: center;
  height: 100%;
  min-height: 0;
  overflow-y: auto;
  padding: var(--md-space-6) var(--md-space-4);
}

.launch-column {
  width: 100%;
  max-width: 640px;
  display: flex;
  flex-direction: column;
  gap: var(--md-space-4);
}

.launch-head {
  display: flex;
  align-items: center;
}

.launch-title {
  margin: 0;
  font-size: var(--md-text-xl);
  font-weight: 700;
}

.launch-spacer {
  flex: 1;
}

.launch-icon {
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  padding: var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.launch-icon:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.launch-modes {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  background: var(--md-bg-sunken);
  border: 1px solid var(--md-border-strong);
  border-radius: var(--md-radius-md);
  align-self: flex-start;
}

.mode {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-1);
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  font-size: var(--md-text-sm);
  padding: 3px var(--md-space-3);
  border-radius: calc(var(--md-radius-md) - 2px);
}

.mode.active {
  background: var(--md-bg-surface);
  color: var(--md-fg);
  box-shadow: var(--md-shadow-1);
}

.launch-form {
  display: flex;
  gap: var(--md-space-2);
}

.launch-input {
  flex: 1;
  min-width: 0;
  height: 30px;
  font-size: var(--md-text-sm);
}

.launch-browse,
.launch-go {
  flex: none;
  height: 30px;
}

.launch-go {
  background: var(--md-accent);
  border-color: var(--md-accent);
  color: var(--md-accent-contrast);
  font-weight: 600;
}

.launch-go:hover:not(:disabled) {
  background: var(--md-accent-strong);
}

.launch-error {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  margin: 0;
  padding: var(--md-space-2) var(--md-space-3);
  background: var(--md-bg-danger-soft);
  border-radius: var(--md-radius-md);
  font-size: var(--md-text-sm);
  color: var(--md-fg);
}

.launch-error :deep(svg) {
  color: var(--md-danger);
}

.launch-error-detail {
  color: var(--md-fg-muted);
  font-size: var(--md-text-xs);
}

.launch-recents,
.launch-nearby {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
}

.recents-head,
.nearby-head {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
}

.recents-title {
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.recents-filter {
  max-width: 200px;
}

.recents-list,
.nearby-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.recents-list li {
  display: flex;
  align-items: center;
}

.recent-row {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  border: none;
  background: transparent;
  border-radius: var(--md-radius-md);
  padding: var(--md-space-2);
  text-align: left;
}

.recent-row:hover {
  background: var(--md-bg-hover);
}

.recent-icon {
  color: var(--md-accent);
}

.recent-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.recent-name {
  font-size: var(--md-text-sm);
  font-weight: 500;
}

.recent-path {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.recent-actions {
  display: inline-flex;
  gap: 2px;
  opacity: 0;
}

.recents-list li:hover .recent-actions {
  opacity: 1;
}

.recent-action {
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  padding: var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.recent-action:hover {
  background: var(--md-bg-active);
  color: var(--md-fg);
}

.nearby-refresh {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  color: var(--md-accent);
  font-size: var(--md-text-xs);
}

.nearby-refresh:hover {
  background: transparent;
}

.nearby-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  border: none;
  background: transparent;
  border-radius: var(--md-radius-sm);
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
  text-align: left;
  color: var(--md-fg-muted);
}

.nearby-row:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

/* Name first, path after it — the same shape as a recents row, so "nearby"
   reads as the same kind of list rather than as a wall of paths. The path
   truncates; the name is what identifies the repository. */
.nearby-name {
  flex: none;
  font-weight: 500;
}

.nearby-path {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--md-fg-subtle);
  font-size: var(--md-text-xs);
}
</style>
