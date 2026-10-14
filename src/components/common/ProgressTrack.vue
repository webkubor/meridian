<script setup lang="ts">
import { computed } from "vue";

/**
 * `<ProgressTrack>` — activity progress (docs/04 §6, §S12): indeterminate,
 * determinate and stalled. `role="progressbar"` with live phase text
 * (docs/04 §9).
 */
const props = withDefaults(
  defineProps<{
    state: "indeterminate" | "determinate" | "stalled";
    done?: number;
    total?: number;
    phase?: string | null;
  }>(),
  { done: 0, total: 0 },
);

const percent = computed(() => {
  if (props.state !== "determinate" || props.total <= 0) return 0;
  return Math.min(100, Math.round((props.done / props.total) * 100));
});

const valueText = computed(() => {
  if (props.state === "stalled") return "Stalled";
  if (props.state === "determinate") return `${props.done} of ${props.total}`;
  return "Working";
});
</script>

<template>
  <div class="progress" :class="state">
    <div class="progress-head">
      <span class="progress-phase">{{ phase ?? valueText }}</span>
      <span v-if="state === 'determinate'" class="progress-count">{{ percent }}%</span>
    </div>
    <div
      class="track"
      role="progressbar"
      :aria-valuemin="0"
      :aria-valuemax="state === 'determinate' ? total : undefined"
      :aria-valuenow="state === 'determinate' ? done : undefined"
      :aria-valuetext="valueText"
    >
      <div v-if="state === 'determinate'" class="fill" :style="{ width: `${percent}%` }" />
      <div v-else class="fill indeterminate" />
    </div>
  </div>
</template>

<style scoped>
.progress {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
  min-width: 160px;
}

.progress-head {
  display: flex;
  justify-content: space-between;
  gap: var(--md-space-2);
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}

.track {
  height: 4px;
  border-radius: 2px;
  background: var(--md-bg-sunken);
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--md-accent);
  border-radius: 2px;
  transition: width var(--md-motion-base) ease;
}

.stalled .fill {
  background: var(--md-warning);
}

.stalled .progress-phase {
  color: var(--md-warning);
}

.fill.indeterminate {
  width: 35%;
  animation: md-indeterminate 1.1s ease-in-out infinite;
}

@keyframes md-indeterminate {
  0% {
    transform: translateX(-110%);
  }
  100% {
    transform: translateX(320%);
  }
}
</style>
