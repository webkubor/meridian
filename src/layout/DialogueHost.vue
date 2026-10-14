<script setup lang="ts">
import { computed } from "vue";

import type { DialogueRequest, DialogueResult } from "../components/contracts";
import ChooseBranchDialog from "../components/dialogues/ChooseBranchDialog.vue";
import ConfirmDialog from "../components/dialogues/ConfirmDialog.vue";
import PromptDialog from "../components/dialogues/PromptDialog.vue";

/**
 * `<DialogueHost>` — renders the shell's one active dialogue from its request
 * (docs/04 §8: one modal at a time). Each kind is narrowed into its own
 * computed so the child's props stay strictly typed; the shell queues further
 * requests behind this one.
 */
const props = defineProps<{
  request: DialogueRequest | null;
  busy?: boolean;
}>();

const emit = defineEmits<{
  resolve: [result: DialogueResult];
  cancel: [id: string];
}>();

const confirmRequest = computed(() =>
  props.request?.kind === "confirm" ? props.request : null,
);
const promptRequest = computed(() => (props.request?.kind === "prompt" ? props.request : null));
const branchRequest = computed(() =>
  props.request?.kind === "chooseBranch" ? props.request : null,
);

function resolve(id: string, kind: DialogueRequest["kind"], value: string, optionChecked: boolean): void {
  emit("resolve", { id, kind, value, optionChecked });
}

function confirmPrompt(value: string, optionChecked: boolean): void {
  const request = promptRequest.value;
  if (!request) return;
  resolve(request.id, "prompt", value, optionChecked);
}

function confirmBranch(name: string): void {
  const request = branchRequest.value;
  if (!request) return;
  resolve(request.id, "chooseBranch", name, false);
}
</script>

<template>
  <ConfirmDialog
    v-if="confirmRequest"
    :title="confirmRequest.title"
    :message="confirmRequest.message"
    :items="confirmRequest.items"
    :confirm-label="confirmRequest.confirmLabel"
    :danger="confirmRequest.danger"
    :busy="busy"
    @confirm="resolve(confirmRequest.id, 'confirm', '', false)"
    @cancel="emit('cancel', confirmRequest.id)"
  />

  <PromptDialog
    v-else-if="promptRequest"
    :title="promptRequest.title"
    :subtitle="promptRequest.subtitle"
    :label="promptRequest.label"
    :initial-value="promptRequest.value"
    :placeholder="promptRequest.placeholder"
    :help-text="promptRequest.helpText"
    :validate="promptRequest.validate"
    :option="promptRequest.option"
    :busy="busy"
    @confirm="confirmPrompt"
    @cancel="emit('cancel', promptRequest.id)"
  />

  <ChooseBranchDialog
    v-else-if="branchRequest"
    :title="branchRequest.title"
    :subtitle="branchRequest.subtitle"
    :branches="branchRequest.branches"
    :current="branchRequest.current"
    :busy="busy"
    @confirm="confirmBranch"
    @cancel="emit('cancel', branchRequest.id)"
  />
</template>
