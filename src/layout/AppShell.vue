<script setup lang="ts">
import WindowChrome from "./WindowChrome.vue";

/**
 * `<AppShell>` — the window frame (docs/04 §4): chrome bar (title + toolbar),
 * sidebar, content area, and an overlay layer for toasts/dialogues/palettes.
 * Structure only; every piece of data arrives through slots.
 */
withDefaults(
  defineProps<{
    title: string;
    subtitle?: string | null;
    /** Width of the sidebar in px (resizable, persisted by the shell). */
    sidebarWidth?: number;
  }>(),
  { subtitle: null, sidebarWidth: 216 },
);
</script>

<template>
  <div class="app-shell">
    <WindowChrome :title="title" :subtitle="subtitle">
      <template #toolbar>
        <slot name="toolbar" />
      </template>
    </WindowChrome>

    <div class="app-frame">
      <aside class="app-sidebar" :style="{ width: `${sidebarWidth}px` }">
        <slot name="sidebar" />
      </aside>
      <main class="app-main">
        <slot />
      </main>
    </div>

    <slot name="overlay" />
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  /* Transparent on purpose: this is the element that would cover the Morandi
   * wash on `body::before`. The canvas colour comes from <html>. */
  background: transparent;
}

.app-frame {
  flex: 1;
  min-height: 0;
  display: flex;
  border-top: 1px solid var(--md-border);
}

.app-sidebar {
  flex: none;
  min-width: 0;
  display: flex;
  flex-direction: column;
  /* Real glass — the sidebar is one of the three containers with enough area
   * for frosting to read as an edge rather than as a tinted bar. */
  background: var(--md-glass-fill);
  backdrop-filter: blur(var(--md-glass-blur));
  -webkit-backdrop-filter: blur(var(--md-glass-blur));
  box-shadow: inset -1px 0 0 var(--md-border-inactive), inset 0 1px 0 var(--md-glass-rim);
  border-right: 1px solid var(--md-border);
}

.app-main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  /* 88% rather than fully transparent: the wash tints the working area, but
   * dense commit rows still need a crisp ground to sit on. */
  background: color-mix(in oklch, var(--md-bg-surface) 88%, transparent);
  backdrop-filter: blur(calc(var(--md-glass-blur) / 2));
  -webkit-backdrop-filter: blur(calc(var(--md-glass-blur) / 2));
}
</style>
