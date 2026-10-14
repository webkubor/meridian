<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { ReleaseNotes } from "../../bridges/bindings";
import { parseChangelog, parseInline, safeHref, type ChangelogEntry } from "./changelog";
import type { LoadState } from "../../stores/loadState";
import { type Locale } from "../../i18n";
import EmptyState from "../common/EmptyState.vue";
import SegmentedControl from "../common/SegmentedControl.vue";

/**
 * `<ReleaseNotesPanel>` — the changelog, rendered from the Markdown the Rust
 * side compiles into the binary (`app_release_notes_load`).
 *
 * Nothing here is fetched. Switching language re-reads the other bundled file,
 * so the notes are always on disk and always available offline.
 *
 * The Markdown is parsed into a structure and rendered with elements rather
 * than through `v-html` — see `./changelog.ts` for why the app has no HTML sink
 * at all.
 */
const props = withDefaults(
  defineProps<{
    notes: LoadState<ReleaseNotes>;
    /** The reader's version, marked in the list so they can see where they are. */
    currentVersion?: string | null;
    /**
     * Which language this copy of the notes is in.
     *
     * Separate from the interface language on purpose: someone filing an issue
     * in English may want the English notes while reading the app in Chinese,
     * and both files are already in the binary. `null` while the notes are
     * still loading, so the toggle does not claim a language it has not read.
     */
    notesLocale?: "zh_cn" | "en" | null;
    /** Shown when a build has no embedded notes at all. */
    compact?: boolean;
  }>(),
  { currentVersion: null, notesLocale: null, compact: false },
);

const emit = defineEmits<{
  retry: [];
  openLink: [url: string];
  /** The reader asked for the other language's notes. */
  "set-locale": [locale: "zh-CN" | "en"];
}>();

const { t } = useI18n();

/** The notes' own language as a UI locale, for the toggle's selected side. */
const notesUiLocale = computed<Locale>(() => (props.notesLocale === "en" ? "en" : "zh-CN"));

const languageOptions = computed(() => [
  { id: "zh-CN", label: t("language.zhCN") },
  { id: "en", label: t("language.en") },
]);

const document = computed(() =>
  props.notes.status === "data" ? parseChangelog(props.notes.data.markdown) : null,
);

/** `Unreleased` is a placeholder label, not a version — show it translated. */
function versionLabel(entry: ChangelogEntry): string {
  return entry.unreleased ? t("changelog.unreleased") : entry.version;
}

function isCurrent(entry: ChangelogEntry): boolean {
  return !entry.unreleased && props.currentVersion !== null && entry.version === props.currentVersion;
}

function segments(text: string) {
  return parseInline(text);
}

/** A link only becomes clickable when its scheme is one the app will open. */
function href(url: string): string | null {
  return safeHref(url);
}
</script>

<template>
  <div class="notes">
    <EmptyState
      v-if="notes.status === 'loading'"
      icon="doc.text"
      :title="t('common.loading')"
      compact
    />

    <EmptyState
      v-else-if="notes.status === 'error'"
      icon="exclamationmark.circle"
      :title="t('changelog.failed')"
      :caption="notes.error.message"
      :action-label="t('common.retry')"
      compact
      @action="emit('retry')"
    />

    <EmptyState
      v-else-if="!document || document.entries.length === 0"
      icon="doc.text"
      :title="t('changelog.empty')"
      compact
    />

    <template v-else>
      <header class="notes-head">
        <div class="notes-head-text">
          <h2 class="notes-title">{{ t("changelog.title") }}</h2>
          <p class="notes-subtitle">{{ t("changelog.subtitle") }}</p>
        </div>
        <SegmentedControl
          v-if="notesLocale"
          :model-value="notesUiLocale"
          :options="languageOptions"
          :label="t('changelog.languageOfNotes')"
          @update:model-value="emit('set-locale', $event as Locale)"
        />
      </header>

      <article v-for="entry in document.entries" :key="entry.version" class="release">
        <header class="release-head">
          <h3 class="release-version">{{ versionLabel(entry) }}</h3>
          <span v-if="isCurrent(entry)" class="release-current">{{ t("changelog.thisBuild") }}</span>
          <span v-if="entry.date" class="release-date">
            {{ t("changelog.releasedOn", { date: entry.date }) }}
          </span>
        </header>

        <p v-for="(paragraph, index) in entry.summary" :key="`s${index}`" class="release-summary">
          <template v-for="(segment, at) in segments(paragraph)" :key="at">
            <strong v-if="segment.kind === 'strong'">{{ segment.text }}</strong>
            <code v-else-if="segment.kind === 'code'">{{ segment.text }}</code>
            <a
              v-else-if="segment.kind === 'link' && href(segment.href)"
              class="notes-link"
              :href="href(segment.href) ?? undefined"
              @click.prevent="emit('openLink', segment.href)"
              >{{ segment.text }}</a
            >
            <span v-else-if="segment.kind === 'link'">{{ segment.text }}</span>
            <template v-else>{{ segment.text }}</template>
          </template>
        </p>

        <section v-for="group in entry.groups" :key="group.title" class="release-group">
          <h4 class="group-title">{{ group.title }}</h4>
          <ul class="group-items">
            <li v-for="(item, index) in group.items" :key="index" :class="`depth-${item.depth}`">
              <template v-for="(segment, at) in segments(item.text)" :key="at">
                <strong v-if="segment.kind === 'strong'">{{ segment.text }}</strong>
                <code v-else-if="segment.kind === 'code'">{{ segment.text }}</code>
                <a
                  v-else-if="segment.kind === 'link' && href(segment.href)"
                  class="notes-link"
                  :href="href(segment.href) ?? undefined"
                  @click.prevent="emit('openLink', segment.href)"
                  >{{ segment.text }}</a
                >
                <span v-else-if="segment.kind === 'link'">{{ segment.text }}</span>
                <template v-else>{{ segment.text }}</template>
              </template>
            </li>
          </ul>
        </section>
      </article>

      <p class="notes-footnote">{{ t("changelog.bundledNote") }}</p>
    </template>
  </div>
</template>

<style scoped>
.notes {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-4);
}

.notes-head {
  display: flex;
  align-items: flex-start;
  gap: var(--md-space-3);
}

.notes-head-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.notes-title {
  margin: 0;
  font-size: var(--md-text-lg);
  font-weight: 600;
}

.notes-subtitle {
  margin: 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg-subtle);
}

.release {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
  padding-bottom: var(--md-space-4);
  border-bottom: 1px solid var(--md-border-inactive);
}

.release-head {
  display: flex;
  align-items: baseline;
  gap: var(--md-space-2);
}

.release-version {
  margin: 0;
  font-size: var(--md-text-md);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.release-current {
  flex: none;
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--md-accent-strong);
  background: var(--md-badge-bg);
  border-radius: var(--md-radius-sm);
  padding: 0 4px;
}

.release-date {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.release-summary {
  margin: 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg-muted);
  max-width: 68ch;
}

.release-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.group-title {
  margin: 0;
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.group-items {
  margin: 0;
  padding-left: var(--md-space-4);
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: var(--md-text-sm);
  max-width: 72ch;
}

.group-items li.depth-1 {
  margin-left: var(--md-space-3);
  color: var(--md-fg-muted);
}

.group-items code {
  background: var(--md-badge-bg);
  border-radius: var(--md-radius-sm);
  padding: 0 3px;
  font-size: 0.92em;
}

.notes-link {
  color: var(--md-accent);
}

.notes-footnote {
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
}
</style>
