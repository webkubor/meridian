<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type {
  DiffDocument,
  DiffOptions,
  DiffRow,
  DiffViewMode,
  HunkAction,
  HunkRef,
  LoadState,
  WordSpan,
} from "../contracts";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import SegmentedControl from "../common/SegmentedControl.vue";
import SwitchToggle from "../common/SwitchToggle.vue";
import { formatBytes } from "../../i18n";
import HunkControls from "./HunkControls.vue";
import { createHighlighter, languageForPath, type HighlightToken } from "./highlight";

/**
 * `<DiffView>` — the diff document renderer (docs/04 §6, §S6/S8).
 *
 * Rows are `HunkHeader | Context | Added | Removed | NoNewline` (docs/02 §13);
 * the viewer renders unified and split layouts over the same document, offers
 * per-hunk stage/unstage/discard (`stageChunk:` family), and owns the
 * large-diff gate. It never computes diffs — the store hands it a document.
 *
 * States: loading / text / binary / image / too-large / error / unchanged.
 */
const props = withDefaults(
  defineProps<{
    state: LoadState<DiffDocument>;
    mode: DiffViewMode;
    options: DiffOptions;
    /** Shown when no document is loaded yet (the selected path). */
    path?: string | null;
    conflicted?: boolean;
    showToolbar?: boolean;
    /** False disables the hunk verb row (read-only perspectives). */
    hunkActions?: boolean;
  }>(),
  {
    path: null,
    conflicted: false,
    showToolbar: true,
    hunkActions: true,
  },
);

const emit = defineEmits<{
  "update:mode": [mode: DiffViewMode];
  "update:options": [options: DiffOptions];
  hunkAction: [payload: { action: HunkAction; hunk: HunkRef; path: string }];
  retry: [];
  showAnyway: [];
}>();

const { t } = useI18n();

/** The conflicted-file reason for the hunk buttons, in the active language. */
const conflictedReason = computed(() =>
  props.conflicted ? t("diff.controlsDisabledReasonHunks") : undefined,
);

const document = computed<DiffDocument | null>(() =>
  props.state.status === "data" ? props.state.data : null,
);

const displayPath = computed(() => {
  const doc = document.value;
  if (!doc) return props.path ?? "";
  if (doc.oldPath && doc.oldPath !== doc.path) return `${doc.oldPath} → ${doc.path}`;
  return doc.path;
});

const language = computed(() => languageForPath(document.value?.path ?? props.path));

const tokensByRow = computed<Map<DiffRow, HighlightToken[]>>(() => {
  const map = new Map<DiffRow, HighlightToken[]>();
  const doc = document.value;
  if (!doc || !props.options.syntax) return map;
  const highlight = createHighlighter(language.value);
  for (const row of doc.rows) {
    if (row.kind === "context" || row.kind === "added" || row.kind === "removed") {
      map.set(row, highlight(row.text));
    }
  }
  return map;
});

interface Cell {
  kind: "context" | "added" | "removed";
  oldLine: number | null;
  newLine: number | null;
  text: string;
  spans: WordSpan[] | null;
  tokens: HighlightToken[] | null;
}

type LineRow = Extract<DiffRow, { kind: "context" | "added" | "removed" }>;

type UnifiedRow =
  | { kind: "header"; header: string; hunk: HunkRef }
  | { kind: "line"; cell: Cell }
  | { kind: "note" };

type SplitRow =
  | { kind: "header"; header: string; hunk: HunkRef }
  | { kind: "pair"; left: Cell | null; right: Cell | null }
  | { kind: "note" };

function cellOf(row: LineRow): Cell {
  return {
    kind: row.kind,
    oldLine: row.oldLine,
    newLine: row.newLine,
    text: row.text,
    spans: row.spans ?? null,
    tokens: tokensByRow.value.get(row) ?? null,
  };
}

const unifiedRows = computed<UnifiedRow[]>(() => {
  const doc = document.value;
  if (!doc) return [];
  return doc.rows.map((row) => {
    if (row.kind === "hunkHeader") return { kind: "header", header: row.header, hunk: row.hunk };
    if (row.kind === "noNewline") return { kind: "note" };
    return { kind: "line", cell: cellOf(row) };
  });
});

/**
 * Split layout: pair each removed-run with the following added-run (the
 * convention every two-column diff uses); leftovers get an empty side.
 */
const splitRows = computed<SplitRow[]>(() => {
  const doc = document.value;
  if (!doc) return [];
  const out: SplitRow[] = [];
  let index = 0;
  while (index < doc.rows.length) {
    const row = doc.rows[index];
    if (row.kind === "hunkHeader") {
      out.push({ kind: "header", header: row.header, hunk: row.hunk });
      index += 1;
      continue;
    }
    if (row.kind === "noNewline") {
      out.push({ kind: "note" });
      index += 1;
      continue;
    }
    if (row.kind === "context") {
      const cell = cellOf(row);
      out.push({
        kind: "pair",
        left: { ...cell, newLine: null },
        right: cell,
      });
      index += 1;
      continue;
    }
    const removed: LineRow[] = [];
    const added: LineRow[] = [];
    while (index < doc.rows.length) {
      const changed = doc.rows[index];
      if (changed.kind !== "removed" && changed.kind !== "added") break;
      if (changed.kind === "removed") removed.push(changed);
      else added.push(changed);
      index += 1;
    }
    const pairs = Math.max(removed.length, added.length);
    for (let step = 0; step < pairs; step += 1) {
      out.push({
        kind: "pair",
        left: removed[step] ? cellOf(removed[step]) : null,
        right: added[step] ? cellOf(added[step]) : null,
      });
    }
  }
  return out;
});

const direction = computed<"stage" | "unstage" | "both" | "none">(() => {
  const doc = document.value;
  if (!doc || !props.hunkActions || props.conflicted) return "none";
  if (doc.perspective === "workdir-vs-index") return "stage";
  if (doc.perspective === "index-vs-head") return "unstage";
  return "none";
});

function onMode(value: string): void {
  emit("update:mode", value === "split" ? "split" : "unified");
}

function updateOption<K extends keyof DiffOptions>(key: K, value: DiffOptions[K]): void {
  emit("update:options", { ...props.options, [key]: value });
}

function onContextChange(event: Event): void {
  const value = Number((event.target as HTMLSelectElement).value);
  updateOption("contextLines", Number.isFinite(value) ? value : props.options.contextLines);
}

function onHunkAction(hunk: HunkRef, action: HunkAction): void {
  const path = document.value?.path ?? props.path;
  if (!path) return;
  emit("hunkAction", { action, hunk, path });
}

function lineLabel(cell: Cell): string {
  const role =
    cell.kind === "added"
      ? t("diff.roleAdded")
      : cell.kind === "removed"
        ? t("diff.roleRemoved")
        : t("diff.roleContext");
  const line = cell.kind === "added" ? cell.newLine : cell.oldLine;
  const where =
    line === null || line === undefined
      ? t("diff.lineNumberUnknown")
      : t("diff.lineNumber", { number: line });
  return t("diff.lineLabel", { where, role, text: cell.text });
}

/** Byte counts go through the shared formatter so the unit follows the language. */
function sizeLabel(bytes: number | null): string {
  return bytes === null ? "" : ` · ${formatBytes(bytes)}`;
}

const errorPayload = computed(() => (props.state.status === "error" ? props.state.error : null));
</script>

<template>
  <div class="diff-viewer">
    <header v-if="showToolbar" class="diff-toolbar">
      <span class="diff-path mono" :title="displayPath">{{ displayPath }}</span>
      <span v-if="document" class="diff-stats mono">
        <span class="stat-added">+{{ document.stats.additions }}</span>
        <span class="stat-removed">−{{ document.stats.deletions }}</span>
      </span>
      <span class="diff-spacer" />
      <SegmentedControl
        :model-value="mode"
        :options="[
          { id: 'unified', label: t('diff.viewUnified') },
          { id: 'split', label: t('diff.viewSplit') },
        ]"
        :label="t('diff.layoutLabel')"
        @update:model-value="onMode"
      />
      <label class="context-picker">
        <span>{{ t("diff.contextLabel") }}</span>
        <select
          :value="options.contextLines"
          :aria-label="t('diff.contextAria')"
          @change="onContextChange"
        >
          <option v-for="count in [0, 1, 3, 6, 12, 25]" :key="count" :value="count">
            {{ count }}
          </option>
        </select>
      </label>
      <SwitchToggle
        :model-value="options.ignoreWhitespace"
        :label="t('diff.ignoreWhitespaceShort')"
        :aria-label="t('diff.ignoreWhitespace')"
        @update:model-value="updateOption('ignoreWhitespace', $event)"
      />
      <SwitchToggle
        :model-value="options.fullFile"
        :label="t('diff.showFullFileShort')"
        :aria-label="t('diff.showFullFile')"
        @update:model-value="updateOption('fullFile', $event)"
      />
      <SwitchToggle
        :model-value="options.syntax"
        :label="t('diff.syntaxShort')"
        :aria-label="t('diff.syntaxColouring')"
        @update:model-value="updateOption('syntax', $event)"
      />
    </header>

    <div class="diff-body">
      <!-- Loading -->
      <div
        v-if="state.status === 'loading'"
        class="diff-skeleton"
        role="status"
        :aria-label="t('diff.loadingAria')"
      >
        <span v-for="row in 10" :key="row" class="skeleton-row" :style="{ width: `${40 + ((row * 17) % 55)}%` }" />
      </div>

      <!-- Error -->
      <div v-else-if="errorPayload" class="diff-error" role="alert">
        <Icon name="exclamationmark.circle" :size="18" class="tone-danger" />
        <p class="diff-error-message">{{ errorPayload.message }}</p>
        <p v-if="errorPayload.detail" class="diff-error-detail mono">{{ errorPayload.detail }}</p>
        <button type="button" @click="emit('retry')">{{ t("diff.tryAgain") }}</button>
      </div>

      <!-- Empty / unchanged -->
      <EmptyState
        v-else-if="!document"
        icon="empty.unchangedFile"
        :title="path ? t('diff.emptyPathTitle') : t('diff.emptyNoFileTitle')"
        :caption="path ? t('diff.emptyPathCaption') : t('diff.emptySelectCaption')"
        compact
      />

      <!-- Too large: the gate (docs/04 §S8) -->
      <div v-else-if="document.tooLarge" class="diff-gate" role="status">
        <Icon name="exclamationmark.triangle" :size="20" class="tone-warning" />
        <p class="diff-gate-title">{{ t("diff.gateTitle") }}</p>
        <p class="diff-gate-caption">
          {{
            t("diff.gateCaption", {
              size: formatBytes(document.tooLarge.bytes),
              limit: formatBytes(document.tooLarge.gate),
            })
          }}
        </p>
        <button type="button" @click="emit('showAnyway')">{{ t("diff.gateAction") }}</button>
      </div>

      <!-- Binary -->
      <EmptyState
        v-else-if="document.binary"
        icon="doc"
        :title="t('diff.binaryTitle')"
        :caption="t('diff.binaryCaption')"
        compact
      />

      <!-- Image -->
      <div v-else-if="document.image" class="diff-image">
        <p class="diff-image-title">{{ t("diff.imageTitle") }}</p>
        <div class="diff-image-pair">
          <figure class="image-tile">
            <span class="checker" aria-hidden="true" />
            <figcaption>
              {{ t("diff.before") }}{{ sizeLabel(document.image.oldSize) }}
            </figcaption>
          </figure>
          <figure class="image-tile">
            <span class="checker" aria-hidden="true" />
            <figcaption>
              {{ t("diff.after") }}{{ sizeLabel(document.image.newSize) }}
            </figcaption>
          </figure>
        </div>
      </div>

      <!-- Text document -->
      <div v-else class="diff-doc mono" :class="`mode-${mode}`">
        <template v-if="mode === 'unified'">
          <div
            v-for="(line, rowIndex) in unifiedRows"
            :key="rowIndex"
            class="diff-row"
            :class="line.kind === 'line' ? `is-${line.cell.kind}` : line.kind"
          >
            <template v-if="line.kind === 'header'">
              <span class="gutter" />
              <span class="gutter" />
              <span class="hunk-header">{{ line.header }}</span>
              <HunkControls
                :hunk="line.hunk"
                :direction="direction"
                :disabled="conflicted"
                :disabled-reason="conflictedReason"
                @action="onHunkAction(line.hunk, $event)"
              />
            </template>
            <template v-else-if="line.kind === 'note'">
              <span class="diff-note">{{ t("diff.noNewline") }}</span>
            </template>
            <template v-else>
              <span class="gutter" aria-hidden="true">{{ line.cell.oldLine ?? "" }}</span>
              <span class="gutter" aria-hidden="true">{{ line.cell.newLine ?? "" }}</span>
              <span class="marker" aria-hidden="true">{{
                line.cell.kind === "added" ? "+" : line.cell.kind === "removed" ? "−" : " "
              }}</span>
              <span class="diff-text" :aria-label="lineLabel(line.cell)">
                <template v-if="line.cell.tokens">
                  <span
                    v-for="(token, tokenIndex) in line.cell.tokens"
                    :key="tokenIndex"
                    :class="`tok-${token.kind}`"
                    >{{ token.text }}</span
                  >
                </template>
                <template v-else-if="line.cell.spans">
                  <span
                    v-for="(span, spanIndex) in line.cell.spans"
                    :key="spanIndex"
                    :class="{ emph: span.emphasized }"
                    >{{ span.text }}</span
                  >
                </template>
                <template v-else>{{ line.cell.text }}</template>
              </span>
            </template>
          </div>
        </template>

        <template v-else>
          <div
            v-for="(line, rowIndex) in splitRows"
            :key="rowIndex"
            class="diff-row split"
            :class="line.kind"
          >
            <template v-if="line.kind === 'header'">
              <span class="gutter" />
              <span class="hunk-header">{{ line.header }}</span>
              <span class="gutter" />
              <span class="hunk-header" />
              <HunkControls
                :hunk="line.hunk"
                :direction="direction"
                :disabled="conflicted"
                :disabled-reason="conflictedReason"
                @action="onHunkAction(line.hunk, $event)"
              />
            </template>
            <template v-else-if="line.kind === 'note'">
              <span class="diff-note">{{ t("diff.noNewline") }}</span>
            </template>
            <template v-else>
              <span class="gutter" aria-hidden="true">{{ line.left?.oldLine ?? "" }}</span>
              <span
                v-if="line.left"
                class="diff-text side-left"
                :class="`is-${line.left.kind}`"
                :aria-label="lineLabel(line.left)"
              >
                <template v-if="line.left.tokens">
                  <span
                    v-for="(token, tokenIndex) in line.left.tokens"
                    :key="tokenIndex"
                    :class="`tok-${token.kind}`"
                    >{{ token.text }}</span
                  >
                </template>
                <template v-else-if="line.left.spans">
                  <span
                    v-for="(span, spanIndex) in line.left.spans"
                    :key="spanIndex"
                    :class="{ emph: span.emphasized }"
                    >{{ span.text }}</span
                  >
                </template>
                <template v-else>{{ line.left.text }}</template>
              </span>
              <span v-else class="diff-text side-left empty-side" aria-hidden="true" />
              <span class="gutter divider" aria-hidden="true">{{ line.right?.newLine ?? "" }}</span>
              <span
                v-if="line.right"
                class="diff-text side-right"
                :class="`is-${line.right.kind}`"
                :aria-label="lineLabel(line.right)"
              >
                <template v-if="line.right.tokens">
                  <span
                    v-for="(token, tokenIndex) in line.right.tokens"
                    :key="tokenIndex"
                    :class="`tok-${token.kind}`"
                    >{{ token.text }}</span
                  >
                </template>
                <template v-else-if="line.right.spans">
                  <span
                    v-for="(span, spanIndex) in line.right.spans"
                    :key="spanIndex"
                    :class="{ emph: span.emphasized }"
                    >{{ span.text }}</span
                  >
                </template>
                <template v-else>{{ line.right.text }}</template>
              </span>
              <span v-else class="diff-text side-right empty-side" aria-hidden="true" />
            </template>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.diff-viewer {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  background: var(--md-bg-surface);
}

.diff-toolbar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-1) var(--md-space-2);
  border-bottom: 1px solid var(--md-border);
  background: var(--md-bg-header);
  font-size: var(--md-text-xs);
  flex-wrap: wrap;
}

.diff-path {
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  max-width: 40%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.diff-stats {
  display: inline-flex;
  gap: var(--md-space-1);
  font-variant-numeric: tabular-nums;
}

.stat-added {
  color: var(--md-success);
}

.stat-removed {
  color: var(--md-danger);
}

.diff-spacer {
  flex: 1;
}

.context-picker {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-1);
  color: var(--md-fg-muted);
}

.context-picker select {
  padding: 0 var(--md-space-1);
  height: 22px;
  font-size: var(--md-text-xs);
}

.diff-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.diff-doc {
  font-size: var(--md-font-size-editor);
  line-height: var(--md-editor-leading);
  padding: var(--md-space-1) 0 var(--md-space-4);
}

.diff-row {
  display: flex;
  align-items: baseline;
  min-height: calc(var(--md-font-size-editor) * var(--md-editor-leading));
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.diff-row.is-added,
.diff-text.is-added {
  background: var(--md-diff-added-bg);
}

.diff-row.is-removed,
.diff-text.is-removed {
  background: var(--md-diff-deleted-bg);
}

.diff-row.header {
  background: var(--md-bg-sunken);
  color: var(--md-fg-muted);
  position: sticky;
  top: 0;
  z-index: 1;
  padding-right: var(--md-space-2);
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
}

.diff-row.note {
  color: var(--md-fg-subtle);
  font-style: italic;
}

.diff-note {
  padding-left: var(--md-space-3);
}

.gutter {
  flex: none;
  width: var(--md-diff-gutter-width);
  padding-right: var(--md-space-2);
  text-align: right;
  color: var(--md-fg-subtle);
  user-select: none;
  background: var(--md-bg-sunken);
  font-variant-numeric: tabular-nums;
}

.diff-row.is-added .gutter,
.diff-row.is-removed .gutter {
  background: transparent;
}

.marker {
  flex: none;
  width: 14px;
  text-align: center;
  color: var(--md-fg-subtle);
  user-select: none;
}

.hunk-header {
  color: var(--md-fg-muted);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.diff-text {
  flex: 1;
  min-width: 0;
  padding-right: var(--md-space-2);
}

.diff-row.split .diff-text {
  flex: 1 1 50%;
}

.empty-side {
  background: var(--md-bg-alt-row);
}

.diff-text .emph {
  border-radius: 2px;
  background: var(--md-diff-word-added-bg);
}

.diff-row.is-removed .emph,
.diff-text.is-removed .emph {
  background: var(--md-diff-word-deleted-bg);
}

.tok-comment {
  color: var(--md-code-comment);
}

.tok-string {
  color: var(--md-code-string);
}

.tok-keyword {
  color: var(--md-code-keyword);
}

.tok-number {
  color: var(--md-code-number);
}

.diff-skeleton {
  padding: var(--md-space-3);
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
}

.skeleton-row {
  height: 10px;
  border-radius: var(--md-radius-sm);
  background: var(--md-bg-sunken);
}

.diff-error,
.diff-gate {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-6) var(--md-space-4);
  text-align: center;
}

.diff-error-message,
.diff-gate-title {
  margin: 0;
  font-size: var(--md-text-sm);
  font-weight: 600;
}

.diff-error-detail,
.diff-gate-caption {
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.diff-image {
  padding: var(--md-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
  align-items: center;
}

.diff-image-title {
  margin: 0;
  font-size: var(--md-text-sm);
  font-weight: 600;
}

.diff-image-pair {
  display: flex;
  gap: var(--md-space-3);
}

.image-tile {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
  align-items: center;
}

.checker {
  width: 180px;
  height: 120px;
  border: 1px solid var(--md-border);
  border-radius: var(--md-radius-md);
  background-image: linear-gradient(45deg, var(--md-diff-checker-b) 25%, transparent 25%),
    linear-gradient(-45deg, var(--md-diff-checker-b) 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, var(--md-diff-checker-b) 75%),
    linear-gradient(-45deg, transparent 75%, var(--md-diff-checker-b) 75%);
  background-size: 16px 16px;
  background-position: 0 0, 0 8px, 8px -8px, -8px 0;
  background-color: var(--md-diff-checker-a);
}

.image-tile figcaption {
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}
</style>
