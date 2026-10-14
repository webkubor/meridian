<script setup lang="ts">
import { useI18n } from "vue-i18n";

import type { HunkRef, HunkAction } from "../contracts";
import Icon from "../common/Icon.vue";

/**
 * `<HunkControls>` — stage / unstage / discard per hunk (docs/04 §S6 chunk
 * staging; `stageChunk:` / `unstageChunk:` / `discardChunk:`). Hidden for
 * non-text documents and disabled for conflicted files (docs/04 §6).
 *
 * Two call styles are supported because two consumers exist: the diff viewer
 * picks the verb set with `direction` and listens for the single `action`
 * event, while the diff module's hunk-header row passes `staged`/`busy` and
 * listens for the individual `stage` / `unstage` / `discard` events. Both are
 * emitted for every press, so neither consumer needs to know about the other.
 */
const props = withDefaults(
  defineProps<{
    hunk: HunkRef;
    /** Which verbs make sense for this side of the duality. */
    direction?: "stage" | "unstage" | "both" | "none";
    disabled?: boolean;
    disabledReason?: string | null;
    /** Hunk-header row style: true renders the unstage verb in place of stage. */
    staged?: boolean;
    busy?: boolean;
  }>(),
  { direction: "both", disabled: false, disabledReason: null, staged: false, busy: false },
);

const emit = defineEmits<{
  action: [action: HunkAction];
  stage: [];
  unstage: [];
  discard: [];
}>();

const { t } = useI18n();

function stage(): void {
  emit("action", "stage");
  emit("stage");
}

function unstage(): void {
  emit("action", "unstage");
  emit("unstage");
}

function discard(): void {
  emit("action", "discard");
  emit("discard");
}

/** Title text for a verb: the disabled reason when there is one. */
function titleOf(fallback: string): string | undefined {
  return props.disabled ? (props.disabledReason ?? undefined) : fallback;
}
</script>

<template>
  <span v-if="direction !== 'none'" class="hunk-controls">
    <button
      v-if="direction === 'stage' || direction === 'both'"
      type="button"
      class="hunk-button"
      :disabled="disabled || busy"
      :title="titleOf(staged ? t('diff.unstageThisHunk') : t('diff.stageThisHunk'))"
      @click="staged ? unstage() : stage()"
    >
      <Icon :name="staged ? 'minus.circle' : 'plus.circle'" :size="12" />
      <span>{{ staged ? t("diff.unstage") : t("diff.stage") }}</span>
    </button>
    <button
      v-if="direction === 'unstage' || direction === 'both'"
      type="button"
      class="hunk-button"
      :disabled="disabled || busy"
      :title="titleOf(t('diff.unstageThisHunk'))"
      @click="unstage()"
    >
      <Icon name="minus.circle" :size="12" />
      <span>{{ t("diff.unstage") }}</span>
    </button>
    <button
      v-if="direction === 'stage' || direction === 'both'"
      type="button"
      class="hunk-button danger"
      :disabled="disabled || busy"
      :title="titleOf(t('diff.discardThisHunk'))"
      @click="discard()"
    >
      <Icon name="arrow.uturn.backward" :size="12" />
      <span>{{ t("diff.discard") }}</span>
    </button>
  </span>
</template>

<style scoped>
.hunk-controls {
  display: inline-flex;
  gap: var(--md-space-1);
  margin-left: auto;
}

.hunk-button {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  border: 1px solid var(--md-border-strong);
  background: var(--md-bg-surface);
  border-radius: var(--md-radius-sm);
  padding: 0 var(--md-space-1);
  height: 20px;
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}

.hunk-button:hover:not(:disabled) {
  color: var(--md-fg);
  background: var(--md-bg-hover);
}

.hunk-button.danger:hover:not(:disabled) {
  color: var(--md-danger);
  border-color: var(--md-danger);
}

.hunk-button:disabled {
  opacity: 0.55;
}
</style>
