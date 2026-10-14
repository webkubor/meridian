<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";

import type { BlameLines, BlamePopover, LoadState } from "../contracts";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import VirtualList from "../common/VirtualList.vue";

/**
 * `<BlamePane>` — the blame view (docs/04 §S5): a gutter of author/commit
 * hunks tinted in alternating shades beside the file's lines. Clicking an
 * annotation opens the commit popover; the tint alternates per hunk so the
 * gutter never relies on colour alone (docs/04 §9) — every hunk's first line
 * carries the full annotation text.
 */
const props = withDefaults(
  defineProps<{
    state: LoadState<BlameLines>;
    blame: BlameLines | null;
    path: string;
    /** The annotation popover the shell currently shows. */
    popover: BlamePopover | null;
    /** Line the shell asked to highlight (deep link / find). */
    highlightLine?: number | null;
  }>(),
  { highlightLine: null },
);

const emit = defineEmits<{
  /** The line whose annotation was clicked (opens the popover). */
  annotate: [line: number];
  selectLine: [line: number];
  closePopover: [];
  openCommit: [oid: string];
  copyOid: [shortOid: string];
  openInEditor: [];
  openDiff: [];
  followRename: [payload: { oid: string; path: string }];
  retry: [];
}>();

const lines = computed(() => props.blame?.lines ?? []);

/**
 * The annotation popover floats OUTSIDE the scrolling list (a panel inside a
 * 20 px virtual row would be clipped by the scroll container), so we remember
 * the clicked row's viewport rect and position a fixed panel there.
 */
const anchor = ref<{ x: number; y: number } | null>(null);

function openAnnotation(line: number, event: MouseEvent): void {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  anchor.value = { x: rect.left, y: rect.bottom + 4 };
  emit("annotate", line);
}

function closeAnnotation(): void {
  anchor.value = null;
  emit("closePopover");
}

function onDocumentPointerDown(event: PointerEvent): void {
  const target = event.target as HTMLElement | null;
  if (target && target.closest(".blame-annot-panel")) return;
  closeAnnotation();
}

function onDocumentKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape") closeAnnotation();
}

watch(
  () => props.popover,
  (popover) => {
    if (popover) {
      document.addEventListener("pointerdown", onDocumentPointerDown, true);
      document.addEventListener("keydown", onDocumentKeydown);
    } else {
      document.removeEventListener("pointerdown", onDocumentPointerDown, true);
      document.removeEventListener("keydown", onDocumentKeydown);
    }
  },
);

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocumentPointerDown, true);
  document.removeEventListener("keydown", onDocumentKeydown);
});

function tintOf(index: number): "a" | "b" {
  const ids = props.blame?.hunkIds;
  if (!ids) return index % 2 === 0 ? "a" : "b";
  return (ids[index] ?? 0) % 2 === 0 ? "a" : "b";
}

function relTime(seconds: number): string {
  const delta = Date.now() / 1000 - seconds;
  if (delta < 60) return "just now";
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  if (delta < 86400 * 30) return `${Math.floor(delta / 86400)}d ago`;
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(seconds * 1000));
}
</script>

<template>
  <div class="blame-pane">
    <header class="blame-bar">
      <Icon name="nav.blame" :size="14" class="blame-icon" />
      <span class="blame-path" :title="path">{{ path }}</span>
      <span class="blame-spacer" />
      <button type="button" class="blame-verb" @click="emit('openDiff')">
        <Icon name="doc.text" :size="13" />
        Show diff
      </button>
      <button type="button" class="blame-verb" @click="emit('openInEditor')">
        <Icon name="arrow.up.forward.square" :size="13" />
        Open
      </button>
    </header>

    <EmptyState
      v-if="state.status === 'loading'"
      icon="hourglass"
      title="Blaming…"
      compact
    />
    <EmptyState
      v-else-if="state.status === 'error'"
      icon="exclamationmark.circle"
      :title="state.error.message"
      action-label="Try again"
      compact
      @action="emit('retry')"
    />
    <EmptyState
      v-else-if="state.status === 'empty' || lines.length === 0"
      icon="empty.noFileSelected"
      title="Nothing to blame"
      caption="This file has no committed history."
    />

    <VirtualList v-else :count="lines.length" :row-height="20" class="blame-list">
      <template #default="{ index }">
        <div
          class="blame-row"
          :class="[`tint-${tintOf(index)}`, { highlighted: lines[index].line === highlightLine }]"
          role="row"
        >
          <div class="blame-gutter">
            <template v-if="lines[index].hunkStart">
              <button
                type="button"
                class="gutter-annot"
                :title="`${lines[index].shortOid} · ${lines[index].author} · ${relTime(lines[index].time)}`"
                @click="openAnnotation(lines[index].line, $event)"
              >
                <span class="annot-author">{{ lines[index].author }}</span>
                <span class="annot-oid mono">{{ lines[index].shortOid }}</span>
                <span class="annot-time">{{ relTime(lines[index].time) }}</span>
                <Icon name="ellipsis" :size="11" class="annot-more" />
              </button>
            </template>
            <span v-else class="gutter-cont" aria-hidden="true" />
          </div>

          <span class="blame-lineno" aria-hidden="true">{{ lines[index].line }}</span>

          <code
            class="blame-code"
            @click="emit('selectLine', lines[index].line)"
            >{{ lines[index].text }}</code
          >
        </div>
      </template>
    </VirtualList>

    <Teleport to="body">
      <div
        v-if="popover && anchor"
        class="blame-annot-panel"
        role="dialog"
        aria-label="Blame annotation"
        :style="{ left: `${anchor.x}px`, top: `${anchor.y}px` }"
      >
        <p class="annot-message">{{ popover.summary }}</p>
        <p class="annot-meta">
          <span class="mono">{{ popover.shortOid }}</span> · {{ popover.author }} ·
          {{ relTime(popover.time) }}
        </p>
        <div class="annot-actions">
          <button type="button" @click="emit('openCommit', popover.oid)">
            <Icon name="clock" :size="12" />
            Show commit
          </button>
          <button
            v-if="popover"
            type="button"
            @click="emit('copyOid', popover.shortOid)"
          >
            <Icon name="doc.on.doc" :size="12" />
            Copy hash
          </button>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.blame-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

.blame-bar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: var(--md-toolbar-height);
  padding: 0 var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
  background: var(--md-bg-header);
}

.blame-icon {
  color: var(--md-fg-subtle);
}

.blame-path {
  font-size: var(--md-text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.blame-spacer {
  flex: 1;
}

.blame-verb {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  color: var(--md-fg-muted);
  font-size: var(--md-text-xs);
  padding: 2px var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.blame-verb:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.blame-list {
  flex: 1;
  min-height: 0;
}

.blame-row {
  display: flex;
  align-items: stretch;
  height: 20px;
  font-size: var(--md-text-xs);
}

.blame-row.tint-a {
  background: var(--md-blame-tint);
}

.blame-row.tint-b {
  background: var(--md-blame-tint-alt);
}

.blame-row.highlighted {
  outline: 1px solid var(--md-accent);
  outline-offset: -1px;
}

.blame-gutter {
  width: var(--md-blame-gutter-width);
  flex: none;
  display: flex;
  align-items: center;
  padding: 0 var(--md-space-1) 0 var(--md-space-2);
  border-right: 1px solid var(--md-border-inactive);
  overflow: hidden;
}

.gutter-annot {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  width: 100%;
  border: none;
  background: transparent;
  padding: 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  text-align: left;
  border-radius: var(--md-radius-sm);
}

.gutter-annot:hover {
  background: transparent;
  color: var(--md-fg);
}

.annot-author {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.annot-oid {
  flex: none;
  color: var(--md-fg-subtle);
}

.annot-time {
  flex: none;
  color: var(--md-fg-subtle);
}

.annot-more {
  flex: none;
  opacity: 0;
}

.gutter-annot:hover .annot-more {
  opacity: 1;
}

.blame-lineno {
  width: 44px;
  flex: none;
  text-align: right;
  padding: 0 var(--md-space-2) 0 var(--md-space-1);
  color: var(--md-fg-disabled);
  font-variant-numeric: tabular-nums;
  font-family: var(--md-font-mono);
  user-select: none;
}

.blame-code {
  flex: 1;
  min-width: 0;
  padding: 0 var(--md-space-3);
  font-family: var(--md-font-mono);
  font-size: var(--md-font-size-editor);
  line-height: 20px;
  white-space: pre;
  overflow: hidden;
  text-overflow: ellipsis;
}

.blame-annot-panel {
  position: fixed;
  z-index: var(--md-z-popover);
  min-width: 260px;
  max-width: 380px;
  padding: var(--md-space-2);
  background: var(--md-bg-surface);
  border: 1px solid var(--md-border);
  border-radius: var(--md-radius-md);
  box-shadow: var(--md-shadow-2);
}

.annot-message {
  margin: 0 0 var(--md-space-1);
  font-size: var(--md-text-sm);
  white-space: normal;
}

.annot-meta {
  margin: 0 0 var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}

.annot-actions {
  display: flex;
  gap: var(--md-space-1);
}

.annot-actions button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--md-text-xs);
  padding: 2px var(--md-space-1);
}
</style>
