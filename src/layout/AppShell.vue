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
  background: var(--md-bg-canvas);
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
  background: var(--md-bg-header);
  border-right: 1px solid var(--md-border);
}

.app-main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--md-bg-surface);
}
</style>
