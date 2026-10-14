<script setup lang="ts">
import { computed } from "vue";

/**
 * `<ProviderMark>` — our own neutral provider mark (docs/04 §3: monogram tiles
 * in the provider's brand hue). No provider artwork is copied; the tint comes
 * from the token table and the monogram carries the meaning.
 */
const props = withDefaults(
  defineProps<{
    provider: string;
    label: string;
    size?: number;
  }>(),
  { size: 20 },
);

const KNOWN = [
  "github",
  "gitlab",
  "bitbucket",
  "azuredevops",
  "stash",
  "beanstalk",
] as const;

const tint = computed(() => {
  const slug = props.provider.toLowerCase();
  return KNOWN.includes(slug as (typeof KNOWN)[number]) ? slug : "custom";
});

const initials = computed(() => (props.label || props.provider).slice(0, 2).toUpperCase());
</script>

<template>
  <span
    class="provider-mark"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      background: `var(--md-provider-${tint})`,
      color: 'var(--md-provider-mark-fg)',
      fontSize: `${Math.max(8, Math.round(size * 0.42))}px`,
    }"
    role="img"
    :aria-label="label"
  >
    {{ initials }}
  </span>
</template>

<style scoped>
.provider-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--md-radius-sm);
  font-weight: 700;
  letter-spacing: 0.02em;
  flex: none;
}
</style>
