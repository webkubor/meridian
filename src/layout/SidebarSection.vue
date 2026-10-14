<script setup lang="ts">
import type { SidebarSectionModel } from "../components/contracts";
import Icon from "../components/common/Icon.vue";
import SidebarRow from "./SidebarRow.vue";

/**
 * `<SidebarSection>` — a collapsible group of sidebar rows (docs/04 §4.1):
 * Working copy / History / Branches / Remotes / Tags / Stashes. The header
 * toggles collapse; a pseudo-section holds exactly one row and hides its
 * header chrome.
 */
defineProps<{
  section: SidebarSectionModel;
  selectedId: string | null;
}>();

const emit = defineEmits<{
  toggle: [id: string];
  select: [id: string];
  options: [id: string];
  add: [id: string];
  contextmenu: [id: string, event: MouseEvent];
}>();
</script>

<template>
  <section class="sidebar-section" :class="{ collapsed: section.collapsed }">
    <div class="section-header">
      <button
        type="button"
        class="section-toggle"
        :aria-expanded="!section.collapsed"
        @click="emit('toggle', section.id)"
      >
        <Icon
          :name="section.collapsed ? 'chevron.right' : 'chevron.down'"
          :size="11"
          class="section-chevron"
        />
        <Icon :name="section.icon" :size="13" class="section-icon" />
        <span class="section-title">{{ section.title }}</span>
      </button>
      <button
        type="button"
        class="section-add"
        :aria-label="$t('sidebar.addToSection', { name: section.title })"
        @click="emit('add', section.id)"
      >
        <Icon name="plus" :size="12" />
      </button>
    </div>

    <div v-if="!section.collapsed" class="section-rows" role="group" :aria-label="section.title">
      <SidebarRow
        v-for="row in section.rows"
        :key="row.id"
        :row="row"
        :selected="row.id === selectedId"
        :indent="section.id === 'branches' || section.id === 'remotes'"
        @select="emit('select', $event)"
        @options="emit('options', $event)"
        @contextmenu="(id, event) => emit('contextmenu', id, event)"
      />
      <p v-if="section.rows.length === 0" class="section-empty">{{ section.emptyCaption }}</p>
    </div>
  </section>
</template>

<style scoped>
.sidebar-section {
  display: flex;
  flex-direction: column;
  padding: var(--md-space-1) var(--md-space-2);
}

.section-header {
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  min-height: 22px;
}

.section-toggle {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  padding: 2px 0;
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  border-radius: var(--md-radius-sm);
}

.section-toggle:hover {
  color: var(--md-fg-muted);
}

.section-chevron {
  color: var(--md-fg-subtle);
}

.section-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.section-add {
  flex: none;
  display: inline-flex;
  align-items: center;
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  padding: 2px;
  border-radius: var(--md-radius-sm);
  opacity: 0;
}

.sidebar-section:hover .section-add,
.section-add:focus-visible {
  opacity: 1;
}

.section-add:hover {
  background: var(--md-bg-active);
  color: var(--md-fg);
}

.section-rows {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding-top: 1px;
}

.section-empty {
  margin: 0;
  padding: var(--md-space-1) var(--md-space-2) var(--md-space-1) var(--md-space-6);
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
}
</style>
