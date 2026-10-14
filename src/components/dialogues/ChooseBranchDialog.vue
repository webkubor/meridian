<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import type { BranchInfo } from "../../bridges/bindings";
import DialogueShell from "./DialogueShell.vue";
import Icon from "../common/Icon.vue";
import RowBadge from "../common/RowBadge.vue";
import SearchField from "../common/SearchField.vue";

/**
 * `<ChooseBranchDialog>` — pick a branch (docs/04 §8.4: the two-option choose
 * pattern for checkout, compare, merge, rebase). Filtered, keyboard-driven;
 * the current branch is marked and cannot be re-chosen.
 */
const props = withDefaults(
  defineProps<{
    title: string;
    subtitle?: string | null;
    branches: BranchInfo[];
    current: string | null;
    /** Compare needs the full ref name so `main` and `origin/main` stay distinct. */
    emitFullName?: boolean;
    confirmLabel?: string;
    busy?: boolean;
  }>(),
  { subtitle: null, confirmLabel: undefined, busy: false },
);

const emit = defineEmits<{
  confirm: [name: string];
  cancel: [];
}>();

const { t } = useI18n();

/** Resolved here, not in `withDefaults`: a default there cannot follow a
 *  language change made while the dialogue was already mounted. */
const confirmText = computed(() => props.confirmLabel ?? t("dialogues.chooseBranch.confirmLabel"));

const filter = ref("");
const selected = ref<string | null>(null);

const filtered = computed(() => {
  const needle = filter.value.trim().toLowerCase();
  const list = props.branches.filter(
    (branch) => needle === "" || branch.name.toLowerCase().includes(needle),
  );
  return [...list].sort((a, b) => {
    if (a.isRemote !== b.isRemote) return a.isRemote ? 1 : -1;
    return a.name.localeCompare(b.name);
  });
});

function valueOf(branch: BranchInfo): string {
  return props.emitFullName ? branch.fullName : branch.name;
}

function canPick(branch: BranchInfo): boolean {
  return !(branch.isHead && valueOf(branch) === props.current);
}

function pick(branch: BranchInfo): void {
  if (!canPick(branch)) return;
  selected.value = valueOf(branch);
}

function confirm(): void {
  if (selected.value === null) return;
  emit("confirm", selected.value);
}
</script>

<template>
  <DialogueShell :title="title" :subtitle="subtitle" size="m" :busy="busy" @close="emit('cancel')">
    <div class="choose-toolbar">
      <SearchField
        v-model="filter"
        :label="$t('dialogues.chooseBranch.filterLabel')"
        :placeholder="$t('dialogues.chooseBranch.filterPlaceholder')"
      />
    </div>

    <ul class="branch-list" role="listbox" :aria-label="$t('dialogues.chooseBranch.listLabel')">
      <li v-for="branch in filtered" :key="branch.fullName">
        <button
          type="button"
          class="branch-row"
          role="option"
          :aria-selected="selected === valueOf(branch)"
          :class="{ selected: selected === valueOf(branch), disabled: !canPick(branch) }"
          :disabled="!canPick(branch)"
          @click="pick(branch)"
          @dblclick="
            pick(branch);
            confirm();
          "
        >
          <Icon
            :name="branch.isRemote ? 'cloud' : 'arrow.triangle.branch'"
            :size="13"
            class="branch-icon"
          />
          <span class="branch-name">{{ branch.name }}</span>
          <RowBadge v-if="branch.isHead" label="HEAD" tone="info" />
          <span v-if="branch.ahead > 0" class="branch-track">
            <Icon name="arrow.up" :size="10" />{{ branch.ahead }}
          </span>
          <span v-if="branch.behind > 0" class="branch-track">
            <Icon name="arrow.down" :size="10" />{{ branch.behind }}
          </span>
        </button>
      </li>
      <li v-if="filtered.length === 0" class="branch-empty">
        {{ $t('dialogues.chooseBranch.empty', { query: filter }) }}
      </li>
    </ul>

    <template #footer>
      <button type="button" class="btn" @click="emit('cancel')">{{ $t('common.cancel') }}</button>
      <button type="button" class="btn primary" :disabled="busy || selected === null" @click="confirm">
        {{ confirmText }}
      </button>
    </template>
  </DialogueShell>
</template>

<style scoped>
.choose-toolbar {
  display: flex;
  padding-bottom: var(--md-space-2);
}

.branch-list {
  margin: 0;
  padding: 0;
  list-style: none;
  max-height: 320px;
  overflow-y: auto;
  border: 1px solid var(--md-border);
  border-radius: var(--md-radius-md);
}

.branch-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  border: none;
  border-radius: 0;
  background: transparent;
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
  text-align: left;
  color: var(--md-fg);
}

.branch-row:hover:not(:disabled) {
  background: var(--md-bg-hover);
}

.branch-row.selected {
  background: var(--md-bg-selected);
}

.branch-row.disabled {
  color: var(--md-fg-disabled);
}

.branch-icon {
  color: var(--md-fg-subtle);
}

.branch-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.branch-track {
  display: inline-flex;
  align-items: center;
  gap: 1px;
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  font-variant-numeric: tabular-nums;
}

.branch-empty {
  padding: var(--md-space-3);
  text-align: center;
  font-size: var(--md-text-sm);
  color: var(--md-fg-subtle);
}
</style>
