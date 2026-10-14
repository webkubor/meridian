<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import type { DialogueRequest } from "../contracts";
import DialogueShell from "./DialogueShell.vue";

/**
 * `<PromptDialog>` — the single-field prompt (docs/04 §8.3): new branch, new
 * tag, remote name, path. Validation is named (`required` / `refname`) so the
 * request stays serialisable; `refname` mirrors `git check-ref-format`'s core
 * rules. An optional checkbox row (e.g. "Check out after creation") reports
 * back with the value.
 */
const props = withDefaults(
  defineProps<{
    title: string;
    subtitle?: string | null;
    label: string;
    initialValue?: string;
    placeholder?: string;
    helpText?: string;
    validate?: Extract<DialogueRequest, { kind: "prompt" }>["validate"];
    option?: { label: string; checked: boolean; helpText?: string };
    confirmLabel?: string;
    busy?: boolean;
  }>(),
  {
    subtitle: null,
    initialValue: "",
    placeholder: "",
    helpText: "",
    validate: undefined,
    option: undefined,
    confirmLabel: undefined,
    busy: false,
  },
);

const emit = defineEmits<{
  confirm: [value: string, optionChecked: boolean];
  cancel: [];
}>();

const { t } = useI18n();

const value = ref(props.initialValue);
const optionChecked = ref(props.option?.checked ?? false);
const touched = ref(false);

const INVALID_CHARS = /[\s~^:?*[\\\x00-\x1f\x7f]/;

function refnameError(text: string): string | null {
  if (text === "") return t("dialogues.prompt.errors.nameRequired");
  if (text === "@") return t("dialogues.prompt.errors.nameAt");
  if (text.startsWith("-")) return t("dialogues.prompt.errors.nameStartsWithDash");
  if (text.startsWith("/") || text.endsWith("/")) return t("dialogues.prompt.errors.nameSlash");
  if (text.startsWith(".") || text.endsWith(".")) return t("dialogues.prompt.errors.nameDot");
  if (text.endsWith(".lock")) return t("dialogues.prompt.errors.nameLock");
  if (text.includes("..")) return t("dialogues.prompt.errors.nameDoubleDot");
  if (text.includes("//")) return t("dialogues.prompt.errors.nameDoubleSlash");
  if (text.includes("@{")) return t("dialogues.prompt.errors.nameAtBrace");
  if (INVALID_CHARS.test(text)) return t("dialogues.prompt.errors.nameInvalidChars");
  return null;
}

const error = computed<string | null>(() => {
  if (props.validate === "required")
    return value.value.trim() === "" ? t("dialogues.prompt.errors.valueRequired") : null;
  if (props.validate === "refname") return refnameError(value.value);
  return null;
});

/** Resolved here rather than in `withDefaults` so it follows the live locale. */
const confirmText = computed(() => props.confirmLabel ?? t("dialogues.prompt.confirmLabel"));

const showError = computed(() => touched.value && error.value !== null);

const canConfirm = computed(() => error.value === null);

function confirm(): void {
  touched.value = true;
  if (!canConfirm.value) return;
  emit("confirm", value.value, optionChecked.value);
}
</script>

<template>
  <DialogueShell :title="title" :subtitle="subtitle" size="s" :busy="busy" @close="emit('cancel')">
    <div class="prompt-field">
      <label class="prompt-label" :for="`prompt-${label}`">{{ label }}</label>
      <input
        :id="`prompt-${label}`"
        v-model="value"
        data-autofocus
        class="prompt-input"
        :class="{ invalid: showError }"
        type="text"
        spellcheck="false"
        autocapitalize="off"
        autocorrect="off"
        :placeholder="placeholder"
        :aria-invalid="showError || undefined"
        :aria-describedby="showError ? `prompt-error-${label}` : undefined"
        @blur="touched = true"
        @keydown.enter.prevent="confirm"
      />
      <p v-if="showError" :id="`prompt-error-${label}`" class="prompt-error" role="alert">
        {{ error }}
      </p>
      <p v-else-if="helpText" class="prompt-help">{{ helpText }}</p>
    </div>

    <label v-if="option" class="prompt-option">
      <input v-model="optionChecked" type="checkbox" />
      <span>
        {{ option.label }}
        <span v-if="option.helpText" class="prompt-option-help">{{ option.helpText }}</span>
      </span>
    </label>

    <template #footer>
      <button type="button" class="btn" @click="emit('cancel')">{{ $t('common.cancel') }}</button>
      <button type="button" class="btn primary" :disabled="busy || !canConfirm" @click="confirm">
        {{ confirmText }}
      </button>
    </template>
  </DialogueShell>
</template>

<style scoped>
.prompt-field {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
}

.prompt-label {
  font-size: var(--md-text-sm);
  color: var(--md-fg-muted);
}

.prompt-input {
  height: 26px;
  padding: 0 var(--md-space-2);
  border: 1px solid var(--md-border-strong);
  border-radius: var(--md-radius-md);
  background: var(--md-bg-surface);
  color: var(--md-fg);
  font-size: var(--md-text-sm);
}

.prompt-input:focus {
  border-color: var(--md-accent);
}

.prompt-input.invalid {
  border-color: var(--md-danger);
}

.prompt-error {
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-danger);
}

.prompt-help {
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.prompt-option {
  display: flex;
  align-items: flex-start;
  gap: var(--md-space-2);
  margin-top: var(--md-space-3);
  font-size: var(--md-text-sm);
  color: var(--md-fg);
}

.prompt-option-help {
  display: block;
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}
</style>
