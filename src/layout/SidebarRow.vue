<script setup lang="ts">
import type { SidebarRowModel } from "../components/contracts";
import Icon from "../components/common/Icon.vue";
import RowBadge from "../components/common/RowBadge.vue";

/**
 * `<SidebarRow>` — one row of the sidebar (docs/04 §4.1): icon, label, badges,
 * and an optional-actions affordance. Selection is a prop, never internal
 * state; the row only reports intent.
 */
const props = withDefaults(
  defineProps<{
    row: SidebarRowModel;
    selected?: boolean;
    /** Nests branch rows under their group header. */
    indent?: boolean;
  }>(),
  { selected: false, indent: false },
);

const emit = defineEmits<{
  select: [id: string];
  options: [id: string];
  contextmenu: [id: string, event: MouseEvent];
}>();

function onContextMenu(event: MouseEvent): void {
  emit("contextmenu", props.row.id, event);
}
</script>

<template>
  <div
    class="sidebar-row"
    :class="{ selected, current: row.current, indented: indent }"
    role="treeitem"
    :aria-selected="selected"
    :aria-current="row.current ? 'true' : undefined"
    @contextmenu.prevent="onContextMenu"
  >
    <button type="button" class="row-main" :title="row.label" @click="emit('select', row.id)">
      <Icon v-if="row.icon" :name="row.icon" :size="14" class="row-icon" />
      <span class="row-label">{{ row.label }}</span>
      <span v-if="row.detail" class="row-detail">{{ row.detail }}</span>
    </button>

    <span v-if="row.badges.length > 0" class="row-badges">
      <RowBadge
        v-for="badge in row.badges"
        :key="badge.id"
        :icon="badge.icon"
        :label="badge.label"
        :tone="badge.tone"
        :selected="selected"
      />
    </span>

    <button
      v-if="row.hasOptions"
      type="button"
      class="row-options"
      :aria-label="$t('sidebar.rowOptions', { name: row.label })"
      @click="emit('options', row.id)"
    >
      <Icon name="ellipsis" :size="14" />
    </button>
  </div>
</template>

<style scoped>
.sidebar-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  min-height: 24px;
  border-radius: var(--md-radius-sm);
  padding: 0 var(--md-space-1) 0 0;
  color: var(--md-fg-muted);
}

.sidebar-row.indented .row-main {
  padding-left: var(--md-space-5);
}

.sidebar-row:hover {
  background: var(--md-bg-hover);
}

.sidebar-row.selected {
  background: var(--md-bg-selected);
  color: var(--md-fg);
}

.sidebar-row.current .row-label {
  font-weight: 600;
}

.row-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  border: none;
  background: transparent;
  color: inherit;
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
  text-align: left;
  border-radius: var(--md-radius-sm);
}

.row-icon {
  color: var(--md-fg-subtle);
}

.sidebar-row.selected .row-icon {
  color: var(--md-accent);
}

.row-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-detail {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
  font-variant-numeric: tabular-nums;
  flex: none;
}

.row-badges {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  flex: none;
}

.row-options {
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

.sidebar-row:hover .row-options,
.sidebar-row.selected .row-options,
.row-options:focus-visible {
  opacity: 1;
}

.row-options:hover {
  background: var(--md-bg-active);
  color: var(--md-fg);
}
</style>
