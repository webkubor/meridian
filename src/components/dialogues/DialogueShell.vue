<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";

import Icon from "../common/Icon.vue";

/**
 * `<DialogueShell>` — the modal frame shared by every dialogue (docs/04 §8):
 * backdrop, sizes S/M/L, focus trap, Escape-to-cancel, and a busy state that
 * blocks dismissal. The primary button colour is decided by the caller
 * (amber/red two-step for destructive verbs, docs/04 §8.2).
 */
const props = withDefaults(
  defineProps<{
    title: string;
    subtitle?: string | null;
    size?: "s" | "m" | "l";
    danger?: boolean;
    busy?: boolean;
    /** Blocks Escape/backdrop dismissal without showing a spinner. */
    blocked?: boolean;
  }>(),
  { subtitle: null, size: "m", danger: false, busy: false, blocked: false },
);

const emit = defineEmits<{ close: [] }>();

const panel = ref<HTMLElement | null>(null);
let previousActive: HTMLElement | null = null;

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function focusables(): HTMLElement[] {
  const element = panel.value;
  if (!element) return [];
  return Array.from(element.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (node) => node.offsetParent !== null || node === document.activeElement,
  );
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape") {
    if (props.busy || props.blocked) return;
    event.stopPropagation();
    emit("close");
    return;
  }
  if (event.key !== "Tab") return;
  const nodes = focusables();
  if (nodes.length === 0) {
    event.preventDefault();
    return;
  }
  const first = nodes[0]!;
  const last = nodes[nodes.length - 1]!;
  const active = document.activeElement as HTMLElement | null;
  if (event.shiftKey && (active === first || !panel.value?.contains(active))) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && active === last) {
    event.preventDefault();
    first.focus();
  }
}

function onBackdrop(): void {
  if (props.busy || props.blocked) return;
  emit("close");
}

onMounted(async () => {
  previousActive = document.activeElement as HTMLElement | null;
  document.addEventListener("keydown", onKeydown, true);
  await nextTick();
  const preferred = panel.value?.querySelector<HTMLElement>("[data-autofocus]");
  (preferred ?? focusables()[0] ?? panel.value)?.focus();
});

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
  previousActive?.focus?.();
});
</script>

<template>
  <Teleport to="body">
    <div class="dialogue-backdrop" @pointerdown.self="onBackdrop">
      <div
        ref="panel"
        class="dialogue"
        :class="[`size-${size}`, { danger }]"
        role="dialog"
        aria-modal="true"
        :aria-label="title"
        tabindex="-1"
      >
        <header class="dialogue-header">
          <div class="dialogue-heading">
            <h2 class="dialogue-title">{{ title }}</h2>
            <p v-if="subtitle" class="dialogue-subtitle">{{ subtitle }}</p>
          </div>
          <button
            type="button"
            class="dialogue-close"
            aria-label="Cancel"
            :disabled="busy || blocked"
            @click="emit('close')"
          >
            <Icon name="xmark" :size="13" />
          </button>
        </header>

        <div class="dialogue-body">
          <slot />
        </div>

        <footer class="dialogue-footer">
          <slot name="footer" />
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.dialogue-backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--md-z-dialog);
  background: var(--md-overlay);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: 12vh var(--md-space-4) var(--md-space-4);
}

.dialogue {
  display: flex;
  flex-direction: column;
  max-height: 76vh;
  background: var(--md-bg-surface);
  border-radius: var(--md-radius-lg);
  box-shadow: var(--md-shadow-2);
  border: 1px solid var(--md-border);
  animation: md-dialogue-in var(--md-motion-base) ease;
  outline: none;
}

.size-s {
  width: 380px;
}

.size-m {
  width: 460px;
}

.size-l {
  width: 640px;
}

.dialogue.danger {
  border-top: 3px solid var(--md-danger);
}

.dialogue-header {
  display: flex;
  align-items: flex-start;
  gap: var(--md-space-2);
  padding: var(--md-space-3) var(--md-space-3) var(--md-space-2);
}

.dialogue-heading {
  flex: 1;
  min-width: 0;
}

.dialogue-title {
  margin: 0;
  font-size: var(--md-text-lg);
  font-weight: 600;
}

.dialogue-subtitle {
  margin: var(--md-space-1) 0 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg-muted);
}

.dialogue-close {
  flex: none;
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  padding: var(--md-space-1);
  border-radius: var(--md-radius-sm);
  display: inline-flex;
}

.dialogue-close:hover:not(:disabled) {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.dialogue-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 var(--md-space-3) var(--md-space-3);
}

.dialogue-footer {
  flex: none;
  display: flex;
  justify-content: flex-end;
  gap: var(--md-space-2);
  padding: var(--md-space-3);
  border-top: 1px solid var(--md-border-inactive);
}

@keyframes md-dialogue-in {
  from {
    opacity: 0;
    transform: translateY(-6px) scale(0.99);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
</style>
