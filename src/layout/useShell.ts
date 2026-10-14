/**
 * Shell state: appearance, toasts, the dialogue queue, and global keyboard
 * shortcuts. Everything here is window-scoped (not repository-scoped), which is
 * why it lives with the layout rather than in a domain store.
 */

import { computed, onBeforeUnmount, onMounted, ref, watch, type WritableComputedRef } from "vue";

import type { AppearanceMode, DialogueRequest, DialogueResult, ToastModel } from "../components/contracts";

/* ---- appearance ---------------------------------------------------------- */

const THEME_KEY = "meridian.appearance";

function readStoredMode(): AppearanceMode {
  // A locked-down webview that refuses reads throws from `getItem`; `i18n`
  // guards the same call, and this runs at module evaluation — an unguarded
  // read here is a blank window with no message.
  try {
    const stored = window.localStorage.getItem(THEME_KEY);
    return stored === "light" || stored === "dark" || stored === "auto" ? stored : "auto";
  } catch {
    return "auto";
  }
}

const themeMode = ref<AppearanceMode>(readStoredMode());
const systemDark = ref(window.matchMedia("(prefers-color-scheme: dark)").matches);

window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (event) => {
  systemDark.value = event.matches;
});

const resolvedTheme = computed<"light" | "dark">(() =>
  themeMode.value === "auto" ? (systemDark.value ? "dark" : "light") : themeMode.value,
);

watch(
  themeMode,
  (mode) => {
    try {
      window.localStorage.setItem(THEME_KEY, mode);
    } catch {
      // Not being able to remember the choice is not a reason to refuse it.
    }
  },
  { immediate: true },
);

watch(
  resolvedTheme,
  (theme) => {
    const root = document.documentElement;
    root.dataset.theme = theme;
    root.classList.toggle("theme-dark", theme === "dark");
    root.style.colorScheme = theme;
  },
  { immediate: true },
);

/* ---- persisted numbers (sidebar / detail widths) ------------------------- */

export function usePersistedNumber(
  key: string,
  initial: number,
  min: number,
  max: number,
): WritableComputedRef<number> {
  let stored = Number.NaN;
  try {
    stored = Number(window.localStorage.getItem(key));
  } catch {
    stored = Number.NaN;
  }
  const value = ref(Number.isFinite(stored) && stored > 0 ? stored : initial);
  watch(value, (next) => {
    try {
      window.localStorage.setItem(key, String(next));
    } catch {
      // Same as the theme: the width simply is not remembered this session.
    }
  });
  return computed({
    get: () => value.value,
    set: (next: number) => {
      value.value = Math.min(max, Math.max(min, next));
    },
  });
}

/* ---- toasts -------------------------------------------------------------- */

const toasts = ref<ToastModel[]>([]);
let toastSeq = 0;
const timers = new Map<string, number>();

export interface ToastInput {
  kind: ToastModel["kind"];
  text: string;
  actionLabel?: string;
  /** Milliseconds before auto-dismiss; ignored for warning/danger (persistent). */
  timeout?: number;
}

function pushToast(input: ToastInput): string {
  const id = `toast-${(toastSeq += 1)}`;
  toasts.value = [...toasts.value, { id, kind: input.kind, text: input.text, actionLabel: input.actionLabel }];
  if (input.kind === "info" || input.kind === "success") {
    const timer = window.setTimeout(() => dismissToast(id), input.timeout ?? 4000);
    timers.set(id, timer);
  }
  return id;
}

function dismissToast(id: string): void {
  const timer = timers.get(id);
  if (timer !== undefined) {
    window.clearTimeout(timer);
    timers.delete(id);
  }
  toasts.value = toasts.value.filter((toast) => toast.id !== id);
}

/* ---- dialogue queue ------------------------------------------------------ */

interface PendingDialogue {
  request: DialogueRequest;
  settle: (result: DialogueResult | null) => void;
}

const queue = ref<PendingDialogue[]>([]);
const activeDialogue = computed<DialogueRequest | null>(() => queue.value[0]?.request ?? null);

export function askDialogue(request: DialogueRequest): Promise<DialogueResult | null> {
  return new Promise((resolve) => {
    queue.value = [...queue.value, { request, settle: resolve }];
  });
}

function settleDialogue(id: string, result: DialogueResult | null): void {
  const head = queue.value[0];
  if (!head || head.request.id !== id) return;
  queue.value = queue.value.slice(1);
  head.settle(result);
}

export function resolveDialogue(result: DialogueResult): void {
  settleDialogue(result.id, result);
}

export function cancelDialogue(id: string): void {
  settleDialogue(id, null);
}

/* ---- keyboard shortcuts -------------------------------------------------- */

export interface ShortcutBinding {
  /** A `KeyboardEvent.key` value, lower-cased (e.g. "k", ",", "enter"). */
  key: string;
  meta?: boolean;
  shift?: boolean;
  alt?: boolean;
  /** Fire even while a text field has focus (default false). */
  allowInInput?: boolean;
  run: () => void;
}

function isTextTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  const tag = element.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || element.isContentEditable;
}

/**
 * Register window-level shortcuts. `meta` matches ⌘ on macOS and Ctrl elsewhere
 * (we treat either modifier as "the platform command key").
 */
export function useKeyboardShortcuts(bindings: ShortcutBinding[]): void {
  function onKeydown(event: KeyboardEvent): void {
    const command = event.metaKey || event.ctrlKey;
    for (const binding of bindings) {
      if (binding.key.toLowerCase() !== event.key.toLowerCase()) continue;
      if (Boolean(binding.meta) !== command) continue;
      if (Boolean(binding.shift) !== event.shiftKey) continue;
      if (Boolean(binding.alt) !== event.altKey) continue;
      if (!binding.allowInInput && isTextTarget(event.target)) continue;
      event.preventDefault();
      binding.run();
      return;
    }
  }

  onMounted(() => document.addEventListener("keydown", onKeydown));
  onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown));
}

/* ---- the composable ------------------------------------------------------ */

export function useShell() {
  return {
    themeMode,
    resolvedTheme,
    setThemeMode: (mode: AppearanceMode) => {
      themeMode.value = mode;
    },
    toasts,
    pushToast,
    dismissToast,
    activeDialogue,
    queuedDialogues: computed(() => Math.max(0, queue.value.length - 1)),
    askDialogue,
    resolveDialogue,
    cancelDialogue,
  };
}
