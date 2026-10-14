<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import type { PaletteItemModel } from "../contracts";
import Icon from "../common/Icon.vue";

/**
 * `<CommandPalette>` — the quick-open / quick-actions palette (docs/04 §S10).
 * The query is a prop and the item list is pre-filtered and ranked by the
 * shell (which may query repositories/files); this component only handles
 * keyboard navigation, grouping, and reporting the choice. ↑/↓ move, Enter
 * runs, Escape closes.
 */
const props = withDefaults(
  defineProps<{
    open: boolean;
    query: string;
    items: PaletteItemModel[];
    placeholder?: string;
    emptyText?: string;
    /** Heading shown above the groups, e.g. "Open repository". */
    heading?: string;
  }>(),
  {
    // Left undefined on purpose: a `withDefaults` default is frozen at mount,
    // and the palette is long-lived — it must relabel itself on a language
    // switch. Resolved in computed props below.
    placeholder: undefined,
    emptyText: undefined,
    heading: "",
  },
);

const emit = defineEmits<{
  "update:query": [value: string];
  select: [id: string];
  close: [];
}>();

const { t } = useI18n();

const placeholderText = computed(() => props.placeholder ?? t("palette.placeholder"));
const emptyMessage = computed(() => props.emptyText ?? t("palette.empty"));

const input = ref<HTMLInputElement | null>(null);
const active = ref(0);

const groups = computed(() => {
  const order: string[] = [];
  const map = new Map<string, PaletteItemModel[]>();
  for (const item of props.items) {
    if (!map.has(item.group)) {
      map.set(item.group, []);
      order.push(item.group);
    }
    map.get(item.group)!.push(item);
  }
  return order.map((group) => ({ group, items: map.get(group)! }));
});

const flat = computed(() => groups.value.flatMap((entry) => entry.items));

watch(
  () => props.items,
  () => {
    active.value = 0;
  },
);

watch(
  () => props.open,
  (open) => {
    if (open) {
      active.value = 0;
      void nextTick(() => input.value?.focus());
      document.addEventListener("keydown", onKeydown, true);
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
);

function onKeydown(event: KeyboardEvent): void {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.stopPropagation();
    event.preventDefault();
    emit("close");
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    if (flat.value.length > 0) active.value = (active.value + 1) % flat.value.length;
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    if (flat.value.length > 0) {
      active.value = (active.value - 1 + flat.value.length) % flat.value.length;
    }
  } else if (event.key === "Enter") {
    event.preventDefault();
    run(flat.value[active.value]);
  }
}

function run(item: PaletteItemModel | undefined): void {
  if (!item || item.disabled) return;
  emit("select", item.id);
}

function indexOf(item: PaletteItemModel): number {
  return flat.value.indexOf(item);
}

onMounted(() => {
  if (props.open) document.addEventListener("keydown", onKeydown, true);
});

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="palette-backdrop" @pointerdown.self="emit('close')">
      <div class="palette" role="dialog" aria-modal="true" :aria-label="$t('palette.label')">
        <div class="palette-search">
          <Icon name="magnifyingglass" :size="14" />
          <input
            ref="input"
            type="text"
            class="palette-input"
            :value="query"
            :placeholder="placeholderText"
            :aria-label="$t('palette.queryLabel')"
            spellcheck="false"
            autocapitalize="off"
            autocorrect="off"
            @input="emit('update:query', ($event.target as HTMLInputElement).value)"
          />
          <kbd class="palette-esc">esc</kbd>
        </div>

        <div class="palette-results" role="listbox" :aria-label="heading || $t('palette.resultsLabel')">
          <template v-for="entry in groups" :key="entry.group">
            <p class="palette-group">{{ entry.group }}</p>
            <button
              v-for="item in entry.items"
              :key="item.id"
              type="button"
              class="palette-item"
              role="option"
              :aria-selected="indexOf(item) === active"
              :class="{ active: indexOf(item) === active, disabled: item.disabled }"
              :disabled="item.disabled"
              :title="item.disabled ? item.reason : item.label"
              @pointermove="active = indexOf(item)"
              @click="run(item)"
            >
              <Icon v-if="item.icon" :name="item.icon" :size="14" class="palette-item-icon" />
              <span class="palette-item-label">{{ item.label }}</span>
              <span v-if="item.detail" class="palette-item-detail">{{ item.detail }}</span>
            </button>
          </template>
          <p v-if="flat.length === 0" class="palette-empty">{{ emptyMessage }}</p>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.palette-backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--md-z-dialog);
  background: var(--md-overlay);
  display: flex;
  justify-content: center;
  padding: 12vh var(--md-space-4) var(--md-space-4);
}

.palette {
  width: 560px;
  max-height: 60vh;
  display: flex;
  flex-direction: column;
  background: var(--md-bg-surface);
  border: 1px solid var(--md-border);
  border-radius: var(--md-radius-lg);
  box-shadow: var(--md-shadow-2);
  overflow: hidden;
  animation: md-palette-in var(--md-motion-base) ease;
}

.palette-search {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-2) var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
  color: var(--md-fg-subtle);
}

.palette-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  padding: 0;
  font-size: var(--md-text-md);
  color: var(--md-fg);
}

.palette-input:focus-visible {
  outline: none;
}

.palette-esc {
  font-family: var(--md-font-mono);
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
  border: 1px solid var(--md-border);
  border-radius: var(--md-radius-sm);
  padding: 0 4px;
}

.palette-results {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--md-space-1);
}

.palette-group {
  margin: 0;
  padding: var(--md-space-2) var(--md-space-2) var(--md-space-1);
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.palette-item {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  border: none;
  border-radius: var(--md-radius-sm);
  background: transparent;
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
  text-align: left;
  color: var(--md-fg);
}

.palette-item.active {
  background: var(--md-bg-selected);
}

.palette-item.disabled {
  color: var(--md-fg-disabled);
}

.palette-item-icon {
  color: var(--md-fg-subtle);
}

.palette-item-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.palette-item-detail {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 50%;
}

.palette-empty {
  margin: 0;
  padding: var(--md-space-4);
  text-align: center;
  font-size: var(--md-text-sm);
  color: var(--md-fg-subtle);
}

@keyframes md-palette-in {
  from {
    opacity: 0;
    transform: translateY(-6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
