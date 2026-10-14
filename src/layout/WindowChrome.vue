<script setup lang="ts">
/**
 * `<WindowChrome>` — the title bar (docs/04 §S2: the title shows the
 * repository name + current branch) with the toolbar row beneath it. The
 * traffic-light inset is a platform affordance, not our drawing.
 */
withDefaults(
  defineProps<{
    title: string;
    subtitle?: string | null;
  }>(),
  { subtitle: null },
);
</script>

<template>
  <header class="chrome">
    <div class="chrome-titlebar">
      <span class="traffic-inset" aria-hidden="true" />
      <div class="chrome-title">
        <span class="title">{{ title }}</span>
        <span v-if="subtitle" class="subtitle">{{ subtitle }}</span>
      </div>
    </div>
    <div class="chrome-toolbar">
      <slot name="toolbar" />
    </div>
  </header>
</template>

<style scoped>
.chrome {
  flex: none;
  background: var(--md-bg-header);
  user-select: none;
}

.chrome-titlebar {
  display: flex;
  align-items: center;
  height: var(--md-chrome-height);
  padding: 0 var(--md-space-3) 0 var(--md-space-2);
  gap: var(--md-space-2);
}

/* Traffic lights are drawn by the window server; we only reserve room. */
.traffic-inset {
  width: 68px;
  flex: none;
}

.chrome-title {
  display: flex;
  align-items: baseline;
  gap: var(--md-space-2);
  min-width: 0;
  flex: 1;
}

.title {
  font-size: var(--md-text-sm);
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.subtitle {
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chrome-toolbar {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: var(--md-toolbar-height);
  padding: 0 var(--md-space-3);
  border-top: 1px solid var(--md-border-inactive);
}
</style>
