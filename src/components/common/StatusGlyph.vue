<script setup lang="ts">
import type { FileStatus, UiStatusView } from "../../bridges/bindings";

/**
 * `<StatusGlyph>` — the file-status cell (docs/04 §S6): shape + colour, so
 * colour is never the only cue (docs/04 §9). The glyph text is `view.short`
 * computed in Rust ("one enum, three renderings") — the UI never derives
 * labels itself.
 */
withDefaults(
  defineProps<{
    view: UiStatusView;
    code: FileStatus;
    /** Dual-axis marker: staged entries render muted, worktree entries solid. */
    staged?: boolean;
  }>(),
  { staged: false },
);

/** Seven families to seven token pairs (renamed folds into added). */
const FAMILY: Record<FileStatus, string> = {
  added: "status-added",
  copied: "status-copied",
  deleted: "status-deleted",
  modified: "status-modified",
  renamed: "status-renamed",
  unmerged: "status-unmerged",
  untracked: "status-untracked",
  ignored: "status-ignored",
};
</script>

<template>
  <span
    class="status-glyph"
    :class="[FAMILY[code], { staged }]"
    :title="view.text"
    :aria-label="view.text"
    role="img"
  >
    {{ view.short }}
  </span>
</template>

<style scoped>
.status-glyph {
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--md-radius-sm);
  font-size: var(--md-text-xs);
  font-weight: 700;
  flex: none;
  font-family: var(--md-font-mono);
}

.status-glyph.staged {
  filter: saturate(0.55);
}

/* Colour is not the only cue: every family also differs in the glyph itself
 * (A/C/D/M/R/U/?), which comes from the Rust-computed `view.short`. */
</style>
