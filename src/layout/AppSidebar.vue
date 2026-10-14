<script setup lang="ts">
import type { RepoSummaryModel, SidebarSectionModel } from "../components/contracts";
import SearchField from "../components/common/SearchField.vue";
import RepoSwitcher from "./RepoSwitcher.vue";
import SidebarSection from "./SidebarSection.vue";

/**
 * `<AppSidebar>` — the repository navigator (docs/04 §4). Composes the
 * repository switcher, the collapsible sections (Working copy, History,
 * Branches, Remotes, Tags, Stashes) and a filter field. All state is props;
 * collapse/selection are owned by the shell.
 */
withDefaults(
  defineProps<{
    current: RepoSummaryModel | null;
    recents: RepoSummaryModel[];
    sections: SidebarSectionModel[];
    selectedId: string | null;
    filter: string;
    locked?: boolean;
  }>(),
  { locked: false },
);

const emit = defineEmits<{
  switchRepo: [id: string];
  openRepositories: [];
  toggleSection: [id: string];
  selectRow: [id: string];
  rowOptions: [id: string];
  sectionAdd: [id: string];
  rowContextmenu: [id: string, event: MouseEvent];
  "update:filter": [value: string];
}>();
</script>

<template>
  <div class="sidebar">
    <RepoSwitcher
      :current="current"
      :recents="recents"
      :locked="locked"
      @switch-repo="emit('switchRepo', $event)"
      @open-repositories="emit('openRepositories')"
    />

    <nav class="sidebar-sections" role="tree" aria-label="Repository">
      <SidebarSection
        v-for="section in sections"
        :key="section.id"
        :section="section"
        :selected-id="selectedId"
        @toggle="emit('toggleSection', $event)"
        @select="emit('selectRow', $event)"
        @options="emit('rowOptions', $event)"
        @add="emit('sectionAdd', $event)"
        @contextmenu="(id, event) => emit('rowContextmenu', id, event)"
      />
    </nav>

    <footer class="sidebar-footer">
      <SearchField
        :model-value="filter"
        label="Filter the sidebar"
        placeholder="Filter"
        @update:model-value="emit('update:filter', $event)"
      />
    </footer>
  </div>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.sidebar-sections {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-bottom: var(--md-space-2);
}

.sidebar-footer {
  flex: none;
  display: flex;
  padding: var(--md-space-1) var(--md-space-2) var(--md-space-2);
  border-top: 1px solid var(--md-border-inactive);
}
</style>
