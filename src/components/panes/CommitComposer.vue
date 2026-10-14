<script setup lang="ts">
import { computed, ref } from "vue";

import type { SplitActionOption } from "../contracts";
import type { StatusSummary } from "../../bridges/bindings";
import Icon from "../common/Icon.vue";
import ProgressTrack from "../common/ProgressTrack.vue";
import SplitAction from "../common/SplitAction.vue";
import SwitchToggle from "../common/SwitchToggle.vue";

/**
 * `<CommitComposer>` — the commit box (docs/04 §S7): subject + body, amend and
 * sign-off toggles, the hook-activity chip, and the changeset summary that
 * makes the *staged* scope explicit. The primary action is a `<SplitAction>`
 * (Commit, with Commit & Push / Commit & … in the caret), and it stays
 * disabled — with a reason — until there is something staged and a subject.
 */
const props = withDefaults(
  defineProps<{
    subject: string;
    body: string;
    summary: StatusSummary;
    amend: boolean;
    signOff: boolean;
    canAmend?: boolean;
    /** Hooks are configured for this repository (shows the chip). */
    hooksEnabled?: boolean;
    /** A hook is currently running. */
    hookRunning?: boolean;
    hookPhase?: string | null;
    busy?: boolean;
    /** Repeated commit attempts with an unchanged message (docs/04 §S7 warn). */
    unchangedMessage?: boolean;
    options?: SplitActionOption[];
  }>(),
  {
    canAmend: true,
    hooksEnabled: false,
    hookRunning: false,
    hookPhase: null,
    busy: false,
    unchangedMessage: false,
    options: undefined,
  },
);

const emit = defineEmits<{
  "update:subject": [value: string];
  "update:body": [value: string];
  "update:amend": [value: boolean];
  "update:signOff": [value: boolean];
  commit: [];
  choose: [optionId: string];
}>();

const bodyOpen = ref(props.body.trim() !== "");

const stagedCount = computed(() => props.summary.staged);
const unstagedCount = computed(() => props.summary.unstaged + props.summary.untracked);

const canCommit = computed(
  () =>
    !props.busy &&
    (props.amend
      ? // An amend may change only the message (nothing staged) or only the
        // staged content (message reused), so either one is enough — but a
        // message-only amend still needs HEAD to exist.
        props.canAmend && (props.subject.trim() !== "" || stagedCount.value > 0)
      : props.subject.trim() !== "" && stagedCount.value > 0),
);

const disabledReason = computed(() => {
  if (props.busy) return "A commit is already running.";
  if (props.amend) {
    if (!props.canAmend) return "There is no commit to amend.";
    if (props.subject.trim() === "" && stagedCount.value === 0) {
      return "Change the message or stage some changes to amend.";
    }
    return undefined;
  }
  if (props.subject.trim() === "") return "Enter a commit subject.";
  if (stagedCount.value === 0) return "Stage some changes to commit.";
  return undefined;
});

function onSubjectInput(event: Event): void {
  emit("update:subject", (event.target as HTMLInputElement).value);
}

function onBodyInput(event: Event): void {
  emit("update:body", (event.target as HTMLTextAreaElement).value);
}
</script>

<template>
  <section class="composer">
    <header class="composer-head">
      <span class="composer-title">{{ amend ? "Amend commit" : "Commit" }}</span>
      <span class="composer-scope">
        <span class="scope-chip staged">
          <Icon name="checkmark.circle" :size="11" />
          {{ stagedCount }} staged
        </span>
        <span v-if="unstagedCount > 0" class="scope-chip">
          {{ unstagedCount }} unstaged
        </span>
      </span>
      <span class="composer-spacer" />
      <span v-if="hooksEnabled" class="hook-chip" :class="{ running: hookRunning }">
        <Icon name="bolt" :size="11" />
        <template v-if="hookRunning">{{ hookPhase ?? "Running hook" }}</template>
        <template v-else>Hooks on</template>
      </span>
    </header>

    <div v-if="hookRunning" class="hook-progress">
      <ProgressTrack state="indeterminate" :phase="hookPhase" />
    </div>

    <div class="composer-fields">
      <input
        class="composer-subject"
        type="text"
        :value="subject"
        :placeholder="amend ? 'Amend the last commit' : 'Commit subject'"
        aria-label="Commit subject"
        spellcheck="false"
        @input="onSubjectInput"
        @keydown.meta.enter="canCommit && emit('commit')"
      />
      <textarea
        v-if="bodyOpen"
        class="composer-body"
        :value="body"
        rows="4"
        placeholder="Describe the change (optional)"
        aria-label="Commit body"
        @input="onBodyInput"
      />
      <button
        v-else
        type="button"
        class="composer-add-body"
        @click="bodyOpen = true"
      >
        <Icon name="text.alignleft" :size="12" />
        Add a description
      </button>
    </div>

    <p v-if="unchangedMessage" class="composer-warn" role="status">
      <Icon name="exclamationmark.triangle" :size="12" />
      The message is unchanged since the last attempt.
    </p>

    <footer class="composer-foot">
      <span class="composer-toggles">
        <SwitchToggle
          :model-value="amend"
          :disabled="!canAmend || busy"
          label="Amend"
          @update:model-value="emit('update:amend', $event)"
        />
        <SwitchToggle
          :model-value="signOff"
          :disabled="busy"
          label="Sign-off"
          @update:model-value="emit('update:signOff', $event)"
        />
      </span>
      <span class="composer-spacer" />
      <SplitAction
        :label="amend ? 'Amend' : 'Commit'"
        icon="checkmark.circle"
        :disabled="!canCommit"
        :reason="disabledReason"
        :busy="busy"
        :options="options"
        @run="emit('commit')"
        @choose="emit('choose', $event)"
      />
    </footer>
  </section>
</template>

<style scoped>
.composer {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
  padding: var(--md-space-2) var(--md-space-3);
  border-top: 1px solid var(--md-border);
  background: var(--md-bg-header);
}

.composer-head {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
}

.composer-title {
  font-size: var(--md-text-sm);
  font-weight: 600;
}

.composer-scope {
  display: inline-flex;
  gap: var(--md-space-1);
}

.scope-chip {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
  background: var(--md-badge-bg);
  border-radius: var(--md-radius-sm);
  padding: 0 var(--md-space-1);
}

.scope-chip.staged {
  color: var(--md-status-added-fg);
  background: var(--md-status-added-bg);
}

.composer-spacer {
  flex: 1;
}

.hook-chip {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.hook-chip.running {
  color: var(--md-accent);
}

.hook-progress {
  padding: 0 var(--md-space-1);
}

.composer-fields {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
}

.composer-subject {
  height: 26px;
  font-size: var(--md-text-sm);
}

.composer-body {
  font-size: var(--md-text-sm);
  resize: vertical;
  min-height: 56px;
  font-family: var(--md-font-ui);
}

.composer-add-body {
  align-self: flex-start;
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  font-size: var(--md-text-xs);
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0;
}

.composer-add-body:hover {
  color: var(--md-accent);
  background: transparent;
}

.composer-warn {
  display: flex;
  align-items: center;
  gap: var(--md-space-1);
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-warning);
}

.composer-foot {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
}

.composer-toggles {
  display: inline-flex;
  gap: var(--md-space-3);
}
</style>
