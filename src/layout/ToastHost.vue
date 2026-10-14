<script setup lang="ts">
import type { ToastModel } from "../components/contracts";
import Icon from "../components/common/Icon.vue";

/**
 * `<ToastHost>` — transient notifications (docs/04 §S11). Mounted once by the
 * shell; it renders whatever the shell's toast queue holds. Danger/warning
 * toasts persist until dismissed (the shell decides lifetime by kind), so this
 * component never auto-hides on its own.
 *
 * `detail` is the failure's machine code, shown small under the sentence: the
 * code is what a bug report quotes, and it is the one part of a toast the user
 * may need to read twice. It arrives from the caller rather than the shell's
 * queue, so the shell keeps owning lifetimes and nothing else.
 */
defineProps<{ toasts: (ToastModel & { detail?: string })[] }>();

const emit = defineEmits<{
  dismiss: [id: string];
  action: [id: string];
}>();

const ICONS: Record<ToastModel["kind"], string> = {
  info: "info.circle",
  success: "checkmark.circle.fill",
  warning: "exclamationmark.triangle",
  danger: "exclamationmark.circle",
};
</script>

<template>
  <Teleport to="body">
    <div class="toast-host" role="status" aria-live="polite">
      <div v-for="toast in toasts" :key="toast.id" class="toast" :class="`kind-${toast.kind}`">
        <Icon :name="ICONS[toast.kind]" :size="14" class="toast-icon" />
        <span class="toast-text">{{ toast.text }}<small v-if="toast.detail">{{ toast.detail }}</small></span>
        <button
          v-if="toast.actionLabel"
          type="button"
          class="toast-action"
          @click="emit('action', toast.id)"
        >
          {{ toast.actionLabel }}
        </button>
        <button
          type="button"
          class="toast-dismiss"
          :aria-label="$t('screens.toast.dismissNotification')"
          @click="emit('dismiss', toast.id)"
        >
          <Icon name="xmark" :size="12" />
        </button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-host {
  position: fixed;
  right: var(--md-space-4);
  bottom: var(--md-space-4);
  z-index: var(--md-z-toast);
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
  align-items: flex-end;
  pointer-events: none;
}

.toast {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-width: 240px;
  max-width: 420px;
  padding: var(--md-space-2) var(--md-space-3);
  background: var(--md-bg-surface);
  border: 1px solid var(--md-border-strong);
  border-left-width: 3px;
  border-radius: var(--md-radius-md);
  box-shadow: var(--md-shadow-2);
  font-size: var(--md-text-sm);
  color: var(--md-fg);
  animation: md-toast-in var(--md-motion-base) ease;
}

.kind-info {
  border-left-color: var(--md-info);
}

.kind-success {
  border-left-color: var(--md-success);
}

.kind-warning {
  border-left-color: var(--md-warning);
}

.kind-danger {
  border-left-color: var(--md-danger);
}

.toast-icon {
  color: var(--md-fg-muted);
}

.kind-success .toast-icon {
  color: var(--md-success);
}

.kind-warning .toast-icon {
  color: var(--md-warning);
}

.kind-danger .toast-icon {
  color: var(--md-danger);
}

.toast-text {
  flex: 1;
  min-width: 0;
}

.toast-text small {
  display: block;
  color: var(--md-fg-subtle);
  font-family: var(--md-font-mono);
  font-size: var(--md-text-xs);
}

.toast-action {
  flex: none;
  border: none;
  background: transparent;
  color: var(--md-accent);
  font-size: var(--md-text-sm);
  font-weight: 600;
  padding: 0;
}

.toast-action:hover {
  text-decoration: underline;
}

.toast-dismiss {
  flex: none;
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  padding: 2px;
  border-radius: var(--md-radius-sm);
  display: inline-flex;
}

.toast-dismiss:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

@keyframes md-toast-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
