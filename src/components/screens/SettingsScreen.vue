<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type {
  AppearanceMode,
  LoadState,
  SettingRowModel,
  SettingValue,
  SettingsDocument,
} from "../contracts";
import { LOCALES, type Locale } from "../../i18n";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import SegmentedControl from "../common/SegmentedControl.vue";
import SwitchToggle from "../common/SwitchToggle.vue";

/**
 * `<SettingsScreen>` — the settings window (docs/04 §S12): a tab list on the
 * left, the active tab's groups on the right. Rows are schema-driven
 * (`settings_get_all`); rows the shell owns until the schema lands are marked
 * "Shell" so nothing pretends to be persisted when it is not.
 */
const props = withDefaults(
  defineProps<{
    document: LoadState<SettingsDocument>;
    activeTab: string;
    /** Shell-owned appearance mode (real today). */
    themeMode: AppearanceMode;
    /** The active interface language. */
    locale: Locale;
    search?: string;
  }>(),
  { search: "" },
);

const emit = defineEmits<{
  "update:activeTab": [tabId: string];
  "update:themeMode": [mode: AppearanceMode];
  "update:locale": [locale: Locale];
  "update:search": [value: string];
  set: [key: string, value: SettingValue];
  reset: [key: string];
  choosePath: [key: string];
  refresh: [];
}>();

const { t } = useI18n();

const tabs = computed(() =>
  props.document.status === "data" ? props.document.data.tabs : [],
);

const active = computed(() => tabs.value.find((tab) => tab.id === props.activeTab) ?? null);

const filteredGroups = computed(() => {
  if (!active.value) return [];
  const needle = props.search.trim().toLowerCase();
  if (needle === "") return active.value.groups;
  return active.value.groups
    .map((group) => ({
      ...group,
      rows: group.rows.filter(
        (row) =>
          row.label.toLowerCase().includes(needle) || row.key.toLowerCase().includes(needle),
      ),
    }))
    .filter((group) => group.rows.length > 0);
});

/**
 * Both switchers build their labels through `t`, so both have to be
 * `computed`: a plain array would freeze the labels at mount and leave the
 * control showing the language that was active when the window opened.
 */
const APPEARANCE_OPTIONS = computed(() => [
  { id: "auto", label: t("screens.settings.appearanceAuto") },
  { id: "light", label: t("screens.settings.appearanceLight") },
  { id: "dark", label: t("screens.settings.appearanceDark") },
]);

const LANGUAGE_OPTIONS = computed(() =>
  LOCALES.map((locale) => ({
    id: locale,
    label: locale === "zh-CN" ? t("language.zhCN") : t("language.en"),
  })),
);

function onBool(row: SettingRowModel, value: boolean): void {
  emit("set", row.key, value);
}

function onNumber(row: SettingRowModel, event: Event): void {
  const raw = Number((event.target as HTMLInputElement).value);
  if (Number.isNaN(raw)) return;
  emit("set", row.key, raw);
}

function onText(row: SettingRowModel, event: Event): void {
  emit("set", row.key, (event.target as HTMLInputElement | HTMLTextAreaElement).value);
}

function onEnum(row: SettingRowModel, event: Event): void {
  emit("set", row.key, (event.target as HTMLSelectElement).value);
}
</script>

<template>
  <div class="settings">
    <nav class="settings-tabs" :aria-label="$t('screens.settings.sectionsLabel')">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        class="settings-tab"
        :class="{ active: tab.id === activeTab }"
        :aria-current="tab.id === activeTab ? 'true' : undefined"
        @click="emit('update:activeTab', tab.id)"
      >
        <Icon :name="tab.icon" :size="14" />
        <span class="settings-tab-label">{{ tab.label }}</span>
        <span v-if="tab.source === 'shell'" class="settings-shell-chip">{{ $t('screens.settings.shellChip') }}</span>
      </button>
    </nav>

    <div class="settings-body">
      <header class="settings-head">
        <h1 class="settings-title">{{ active?.label ?? $t('screens.settings.title') }}</h1>
        <span class="settings-spacer" />
        <input
          class="settings-search"
          type="search"
          :value="search"
          :placeholder="$t('screens.settings.searchPlaceholder')"
          :aria-label="$t('screens.settings.searchLabel')"
          @input="emit('update:search', ($event.target as HTMLInputElement).value)"
        />
      </header>

      <EmptyState
        v-if="document.status === 'loading'"
        icon="hourglass"
        :title="$t('screens.settings.loading')"
        compact
      />
      <EmptyState
        v-else-if="document.status === 'error'"
        icon="exclamationmark.circle"
        :title="document.error.message"
        :action-label="$t('screens.settings.tryAgain')"
        compact
        @action="emit('refresh')"
      />
      <EmptyState
        v-else-if="!active"
        icon="nav.settings"
        :title="$t('screens.settings.emptyTitle')"
        :caption="$t('screens.settings.emptyCaption')"
      />

      <div v-else class="settings-scroll">
        <section v-if="active.id === 'appearance'" class="settings-group">
          <h2 class="group-title">{{ $t('screens.settings.theme') }}</h2>
          <div class="setting-row">
            <div class="row-labels">
              <span class="row-label">{{ $t('screens.settings.appearance') }}</span>
              <span class="row-help">{{ $t('screens.settings.appearanceHelp') }}</span>
            </div>
            <SegmentedControl
              :model-value="themeMode"
              :options="APPEARANCE_OPTIONS"
              :label="$t('screens.settings.appearance')"
              @update:model-value="emit('update:themeMode', $event as AppearanceMode)"
            />
          </div>
        </section>

        <section v-if="active.id === 'appearance'" class="settings-group">
          <h2 class="group-title">{{ t("language.label") }}</h2>
          <div class="setting-row">
            <div class="row-labels">
              <span class="row-label">{{ t("language.label") }}</span>
              <span class="row-help">{{ t("language.note") }}</span>
            </div>
            <SegmentedControl
              :model-value="locale"
              :options="LANGUAGE_OPTIONS"
              :label="t('language.label')"
              @update:model-value="emit('update:locale', $event as Locale)"
            />
          </div>
        </section>

        <section v-for="group in filteredGroups" :key="group.id" class="settings-group">
          <h2 class="group-title">{{ group.label }}</h2>

          <div v-for="row in group.rows" :key="row.key" class="setting-row">
            <div class="row-labels">
              <span class="row-label">
                {{ row.label }}
                <span v-if="row.shellOwned" class="settings-shell-chip">{{ $t('screens.settings.shellChip') }}</span>
              </span>
              <span v-if="row.help" class="row-help">{{ row.help }}</span>
            </div>

            <div class="row-control">
              <SwitchToggle
                v-if="row.kind === 'bool'"
                :model-value="row.value === true"
                :aria-label="row.label"
                @update:model-value="onBool(row, $event)"
              />

              <input
                v-else-if="row.kind === 'int'"
                class="control-input number"
                type="number"
                :value="row.value as number"
                :min="row.min"
                :max="row.max"
                :aria-label="row.label"
                @change="onNumber(row, $event)"
              />

              <textarea
                v-else-if="row.kind === 'text'"
                class="control-input"
                rows="3"
                :value="row.value as string"
                :placeholder="row.placeholder"
                :aria-label="row.label"
                @change="onText(row, $event)"
              />

              <select
                v-else-if="row.kind === 'enum'"
                class="control-input"
                :value="String(row.value)"
                :aria-label="row.label"
                @change="onEnum(row, $event)"
              >
                <option v-for="option in row.options ?? []" :key="option.value" :value="option.value">
                  {{ option.label }}
                </option>
              </select>

              <span v-else-if="row.kind === 'path'" class="control-path">
                <input
                  class="control-input"
                  type="text"
                  :value="row.value as string"
                  :placeholder="row.placeholder"
                  :aria-label="row.label"
                  @change="onText(row, $event)"
                />
                <button type="button" class="control-browse" @click="emit('choosePath', row.key)">
                  {{ $t('screens.choose') }}
                </button>
              </span>

              <input
                v-else-if="row.kind === 'color'"
                class="control-color"
                type="color"
                :value="String(row.value)"
                :aria-label="row.label"
                @change="onText(row, $event)"
              />

              <input
                v-else
                class="control-input"
                type="text"
                :value="row.value as string"
                :placeholder="row.placeholder"
                :aria-label="row.label"
                @change="onText(row, $event)"
              />

              <button
                v-if="row.defaultValue !== undefined && row.value !== row.defaultValue"
                type="button"
                class="row-reset"
                :title="$t('screens.settings.resetTitle')"
                @click="emit('reset', row.key)"
              >
                {{ $t('screens.settings.reset') }}
              </button>
            </div>
          </div>

          <p v-if="group.rows.length === 0" class="group-empty">{{ $t('screens.settings.noMatchingGroup') }}</p>
        </section>

        <EmptyState
          v-if="filteredGroups.length === 0 && active.id !== 'appearance'"
          icon="magnifyingglass"
          :title="$t('screens.settings.noMatchingTitle')"
          :caption="$t('screens.settings.nothingMatches', { query: search })"
          compact
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  min-height: 0;
  flex: 1;
}

.settings-tabs {
  width: 220px;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: var(--md-space-2);
  border-right: 1px solid var(--md-border);
  background: var(--md-bg-header);
  overflow-y: auto;
}

.settings-tab {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  border: none;
  background: transparent;
  border-radius: var(--md-radius-sm);
  padding: var(--md-space-1) var(--md-space-2);
  font-size: var(--md-text-sm);
  color: var(--md-fg-muted);
  text-align: left;
}

.settings-tab:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.settings-tab.active {
  background: var(--md-bg-selected);
  color: var(--md-fg);
  font-weight: 600;
}

.settings-tab-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.settings-shell-chip {
  flex: none;
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--md-fg-subtle);
  background: var(--md-badge-bg);
  border-radius: var(--md-radius-sm);
  padding: 0 4px;
}

.settings-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.settings-head {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: var(--md-toolbar-height);
  padding: 0 var(--md-space-4);
  border-bottom: 1px solid var(--md-border);
}

.settings-title {
  margin: 0;
  font-size: var(--md-text-lg);
  font-weight: 600;
}

.settings-spacer {
  flex: 1;
}

.settings-search {
  width: 220px;
  height: 24px;
  font-size: var(--md-text-sm);
}

.settings-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--md-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--md-space-4);
  max-width: 720px;
}

.settings-group {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
}

.group-title {
  margin: 0 0 var(--md-space-1);
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.setting-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-4);
  padding: var(--md-space-2) 0;
  border-bottom: 1px solid var(--md-border-inactive);
}

.row-labels {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.row-label {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-2);
  font-size: var(--md-text-sm);
}

.row-help {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.row-control {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-2);
  min-width: 220px;
  justify-content: flex-end;
}

.control-input {
  width: 220px;
  height: 24px;
  font-size: var(--md-text-sm);
}

.control-input.number {
  width: 88px;
}

.control-path {
  display: inline-flex;
  gap: var(--md-space-1);
}

.control-browse {
  height: 24px;
  font-size: var(--md-text-sm);
}

.control-color {
  width: 40px;
  height: 24px;
  padding: 2px;
  background: var(--md-bg-surface);
  border: 1px solid var(--md-border-strong);
  border-radius: var(--md-radius-sm);
}

.row-reset {
  border: none;
  background: transparent;
  color: var(--md-accent);
  font-size: var(--md-text-xs);
}

.row-reset:hover {
  background: transparent;
  text-decoration: underline;
}

.group-empty {
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
}
</style>
