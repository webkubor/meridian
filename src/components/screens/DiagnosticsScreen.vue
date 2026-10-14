<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";

import { useI18n } from "vue-i18n";

import type { CrashNote, DebugState, ErrorPayload } from "../../bridges/bindings";
import type { CallRecord, FailureRecord } from "../../diagnostics/recorder";
import { formatDateTime } from "../../i18n";
import { errorText } from "../../i18n";
import Icon from "../common/Icon.vue";
import SwitchToggle from "../common/SwitchToggle.vue";

/**
 * `<DiagnosticsScreen>` — everything the owner needs to make a bug report
 * answerable: the failure in their language, the call that produced it, the log
 * tail around it, and three ways to get it out (copy, file, issue form).
 *
 * Presentational: every command runs in `useDiagnostics`, and the screen asks
 * for a refresh when it opens (the log tail read at launch predates the failure
 * the owner came to look at) and whenever a failure is recorded while it is on
 * screen.
 */
const props = defineProps<{
  debug: DebugState | null;
  crash: CrashNote | null;
  calls: readonly CallRecord[];
  failures: readonly FailureRecord[];
  logTail: string;
  busy: boolean;
  error: ErrorPayload | null;
  /** The failure the route named (`?failure=`), expanded on arrival. */
  focusId: string | null;
  /** The call a failure came from, when the caller already knows it. */
  callFor?: (failure: FailureRecord) => CallRecord | undefined;
}>();
const emit = defineEmits<{
  refresh: [];
  setDebug: [enabled: boolean];
  revealLogs: [];
  dismissCrash: [];
  copyDetails: [failure: FailureRecord | null];
  copyReport: [failure: FailureRecord | null];
  exportReport: [failure: FailureRecord | null];
  openDevtools: [];
  relaunch: [];
  reportProblem: [failure: FailureRecord | null];
}>();
const { t } = useI18n();
const expanded = ref<string | null>(null);

const logElement = ref<HTMLElement | null>(null);

onMounted(() => emit("refresh"));
watch(
  () => props.focusId,
  (id) => {
    if (id) expanded.value = id;
  },
  { immediate: true },
);
// The log opens at the oldest line, which is the least useful one: a reader
// arriving from a toast wants what just happened.
watch(
  () => props.logTail,
  async () => {
    await nextTick();
    if (logElement.value) logElement.value.scrollTop = logElement.value.scrollHeight;
  },
);

function toggle(id: string): void {
  expanded.value = expanded.value === id ? null : id;
}

/** The sentence a person reads: the catalogue's, by class then code. */
function headline(error: ErrorPayload): string {
  return errorText(error);
}

/** Labeled lines: which field is which, so the detail block can be read. */
function errorDetails(failure: FailureRecord): string {
  const call = props.callFor?.(failure);
  const lines = [
    `${t("diagnostics.failures.command")}: ${failure.command}`,
    `${t("diagnostics.failures.occurred")}: ${formatDateTime(failure.at)}`,
    `code: ${failure.error.code}`,
    `domain: ${failure.error.domain}`,
    `message: ${failure.error.message}`,
    failure.error.detail ? `${t("diagnostics.failures.detail")}: ${failure.error.detail}` : "",
    call ? `${t("diagnostics.calls.duration")}: ${call.durationMs} ms` : "",
    call ? `${t("diagnostics.calls.arguments")}: ${call.argKeys.join(", ")}` : "",
  ];
  return lines.filter(Boolean).join("\n");
}

function sourceLabel(source: DebugState["source"]): string {
  return t(`diagnostics.debug.sourceValues.${source}`);
}

const crashTitle = computed(() => {
  switch (props.crash?.kind) {
    case "recovered_panic":
      return t("diagnostics.crash.recoveredTitle");
    case "unclean_exit":
      return t("diagnostics.crash.uncleanTitle");
    default:
      return t("diagnostics.crash.title");
  }
});

const devtoolsEnabled = computed(() => props.debug?.enabled === true);
/** False until a restart: the webview decides inspector support at creation. */
const devtoolsReady = computed(() => props.debug?.devtoolsReady === true);
const logLines = computed(() => props.logTail.split("\n"));
</script>

<template>
  <section class="diagnostics-screen" aria-labelledby="diagnostics-heading">
    <header class="screen-head">
      <div>
        <h1 id="diagnostics-heading">{{ t('diagnostics.title') }}</h1>
        <p>{{ t('diagnostics.description') }}</p>
      </div>
      <button type="button" class="action" :disabled="busy" @click="emit('refresh')">
        <Icon name="arrow.clockwise" :size="14" /> {{ t('common.refresh') }}
      </button>
    </header>

    <p v-if="error" class="error-banner" role="alert">
      {{ headline(error) }}
      <small>{{ error.code }}</small>
    </p>
    <p v-else-if="busy && logLines.length === 0" class="subtle" role="status">{{ t('common.loading') }}</p>

    <section v-if="crash" class="card crash-card">
      <header><Icon name="exclamationmark.triangle" :size="16" /><strong>{{ crashTitle }}</strong>
        <button type="button" class="action" @click="emit('dismissCrash')">{{ t('diagnostics.crash.dismiss') }}</button>
      </header>
      <p>{{ t('diagnostics.crash.kind.' + crash.kind) }} · {{ formatDateTime(crash.at) }}</p>
      <p>{{ crash.message }}</p>
      <p v-if="crash.location"><strong>{{ t('diagnostics.crash.location') }}:</strong> {{ crash.location }}</p>
      <pre v-if="crash.backtraceExcerpt" tabindex="0" :aria-label="t('diagnostics.crash.backtrace')">{{ t('diagnostics.crash.backtrace') }}: {{ crash.backtraceExcerpt }}</pre>
    </section>

    <section class="card">
      <header><Icon name="gearshape" :size="15" /><h2>{{ t('diagnostics.debug.label') }}</h2></header>
      <div class="body">
        <div class="row">
          <div>
            <strong>{{ t('diagnostics.debug.label') }}</strong>
            <p>{{ debug?.enabled ? t('diagnostics.debug.enabled') : t('diagnostics.debug.disabled') }}</p>
            <p v-if="debug" class="subtle">{{ t('diagnostics.debug.source', { source: sourceLabel(debug.source) }) }}</p>
          </div>
          <SwitchToggle
            :model-value="debug?.enabled ?? false"
            :aria-label="t('diagnostics.debug.label')"
            :disabled="busy || debug?.source === 'env'"
            @update:model-value="emit('setDebug', $event)"
          />
        </div>
        <p v-if="debug?.source === 'env'" class="subtle">{{ t('diagnostics.debug.envLocked') }}</p>
        <dl v-if="debug" class="paths">
          <dt>{{ t('diagnostics.logs.directory') }}</dt><dd>{{ debug.logDir }}</dd>
          <dt>{{ t('diagnostics.logs.file') }}</dt><dd>{{ debug.logFile }}</dd>
        </dl>
        <button type="button" class="action" @click="emit('revealLogs')">{{ t('diagnostics.logs.reveal') }}</button>
        <button
          v-if="devtoolsEnabled"
          type="button"
          class="action"
          :disabled="!devtoolsReady"
          :title="devtoolsReady ? undefined : t('diagnostics.devtools.restartHint')"
          @click="emit('openDevtools')"
        >{{ t('diagnostics.actions.openDevtools') }}</button>
        <p v-if="devtoolsEnabled && !devtoolsReady" class="subtle">
          {{ t('diagnostics.devtools.restartHint') }}
          <button type="button" class="link-button" @click="emit('relaunch')">{{ t('diagnostics.devtools.restart') }}</button>
        </p>
      </div>
    </section>

    <section class="card">
      <header><Icon name="exclamationmark.circle" :size="15" /><h2>{{ t('diagnostics.failures.title') }}</h2></header>
      <div v-if="failures.length" class="rows">
        <article v-for="failure in [...failures].reverse()" :key="failure.id" class="failure-row" :data-failure="failure.id">
          <button type="button" class="failure-toggle" :aria-expanded="expanded === failure.id" @click="toggle(failure.id)">
            <span>
              <strong>{{ headline(failure.error) }}</strong>
              <small>{{ failure.error.code }} · {{ failure.command }} · {{ formatDateTime(failure.at) }}</small>
              <small v-if="failure.id === focusId" class="focused">{{ t('diagnostics.failures.focus') }}</small>
            </span>
            <Icon :name="expanded === failure.id ? 'chevron.down' : 'chevron.right'" :size="13" />
          </button>
          <div v-if="expanded === failure.id" class="failure-detail">
            <pre tabindex="0" :aria-label="t('diagnostics.failures.detail')">{{ errorDetails(failure) }}</pre>
            <div class="actions">
              <button type="button" class="action" @click="emit('copyDetails', failure)">{{ t('diagnostics.failures.copy') }}</button>
              <button type="button" class="action" @click="emit('copyReport', failure)">{{ t('diagnostics.failures.copyReport') }}</button>
              <button type="button" class="action" @click="emit('reportProblem', failure)">{{ t('diagnostics.failures.report') }}</button>
            </div>
          </div>
        </article>
      </div>
      <p v-else class="body subtle">{{ t('diagnostics.failures.empty') }}</p>
    </section>

    <section class="card">
      <header><Icon name="clock" :size="15" /><h2>{{ t('diagnostics.calls.title') }}</h2></header>
      <div class="table-scroll" tabindex="0" :aria-label="t('diagnostics.calls.scrollLabel')">
        <table>
          <caption class="subtle">{{ t('diagnostics.calls.caption') }}</caption>
          <thead><tr><th scope="col">{{ t('diagnostics.calls.command') }}</th><th scope="col">{{ t('diagnostics.calls.started') }}</th><th scope="col">{{ t('diagnostics.calls.duration') }}</th><th scope="col">{{ t('diagnostics.calls.outcome') }}</th><th scope="col">{{ t('diagnostics.calls.arguments') }}</th></tr></thead>
          <tbody>
            <tr v-for="(call, index) in [...calls].reverse()" :key="`${call.startedAt}-${call.command}-${index}`">
              <td>{{ call.command }}</td><td>{{ formatDateTime(call.startedAt) }}</td><td>{{ call.durationMs }} ms</td>
              <td>{{ call.ok ? t('diagnostics.calls.success') : `${t('diagnostics.calls.failed')} (${call.errorCode})` }}</td>
              <td>{{ call.argKeys.join(', ') }}</td>
            </tr>
            <tr v-if="calls.length === 0"><td colspan="5" class="subtle">{{ t('diagnostics.calls.empty') }}</td></tr>
          </tbody>
        </table>
      </div>
    </section>

    <section class="card">
      <header><Icon name="doc.text" :size="15" /><h2>{{ t('diagnostics.logs.title') }}</h2>
        <button type="button" class="action" :disabled="busy" @click="emit('refresh')">{{ t('diagnostics.logs.refresh') }}</button>
      </header>
      <pre ref="logElement" class="log-tail" tabindex="0" :aria-label="t('diagnostics.logs.scrollLabel')">{{ logTail || t('diagnostics.logs.empty') }}</pre>
    </section>

    <div class="footer-actions">
      <button type="button" class="action" @click="emit('copyReport', null)">{{ t('diagnostics.actions.copyReport') }}</button>
      <button type="button" class="action" @click="emit('exportReport', null)">{{ t('diagnostics.actions.export') }}</button>
      <button type="button" class="action primary" @click="emit('reportProblem', null)">{{ t('diagnostics.actions.report') }}</button>
    </div>
  </section>
</template>

<style scoped>
.diagnostics-screen { flex: 1; min-height: 0; overflow-y: auto; padding: var(--md-space-4); display: flex; flex-direction: column; gap: var(--md-space-3); max-width: 1000px; }
.screen-head { display: flex; align-items: center; gap: var(--md-space-3); }
.screen-head > div { flex: 1; }
h1, h2, p { margin: 0; }
h1 { font-size: var(--md-text-lg); }
h2 { font-size: var(--md-text-sm); }
.screen-head p, .subtle { color: var(--md-fg-subtle); font-size: var(--md-text-xs); }
.card { border: 1px solid var(--md-border); border-radius: var(--md-radius-md); background: var(--md-bg-surface); overflow: hidden; }
.card > header { min-height: 36px; padding: var(--md-space-2) var(--md-space-3); display: flex; align-items: center; gap: var(--md-space-2); border-bottom: 1px solid var(--md-border-inactive); }
.card > header .action { margin-left: auto; }
.body { padding: var(--md-space-3); display: flex; flex-direction: column; gap: var(--md-space-2); }
.row { display: flex; align-items: center; justify-content: space-between; gap: var(--md-space-3); }
.row p { margin-top: var(--md-space-1); font-size: var(--md-text-xs); color: var(--md-fg-muted); }
.paths { display: grid; grid-template-columns: 110px 1fr; gap: var(--md-space-1) var(--md-space-3); margin: 0; font-size: var(--md-text-xs); }
.paths dt { color: var(--md-fg-subtle); }.paths dd { margin: 0; font-family: var(--md-font-mono); overflow-wrap: anywhere; user-select: text; }
.link-button { border: none; background: transparent; color: var(--md-accent); font-size: inherit; padding: 0; text-decoration: underline; }
.crash-card { border-color: var(--md-danger); }.crash-card header { color: var(--md-danger); }.crash-card header strong { flex: 1; }.crash-card > p, .crash-card > pre { margin: var(--md-space-2) var(--md-space-3); font-size: var(--md-text-sm); white-space: pre-wrap; overflow-wrap: anywhere; }
.error-banner { color: var(--md-danger); font-size: var(--md-text-sm); }.error-banner small { font-family: var(--md-font-mono); }
.rows { display: flex; flex-direction: column; }.failure-row + .failure-row { border-top: 1px solid var(--md-border-inactive); }
.failure-row.focused { background: var(--md-bg-selected); }
.failure-toggle { width: 100%; border: 0; background: transparent; color: var(--md-fg); padding: var(--md-space-2) var(--md-space-3); display: flex; align-items: center; text-align: left; gap: var(--md-space-2); }.failure-toggle > span { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }.failure-toggle strong { overflow: hidden; text-overflow: ellipsis; }.failure-toggle small { color: var(--md-fg-subtle); font-size: var(--md-text-xs); }
.failure-detail { padding: 0 var(--md-space-3) var(--md-space-3); }.failure-detail pre, .log-tail { padding: var(--md-space-2); margin: 0; max-height: 260px; overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; background: var(--md-bg-sunken); color: var(--md-fg-muted); font: var(--md-text-xs)/1.45 var(--md-font-mono); user-select: text; }
.table-scroll { overflow: auto; max-height: 320px; }table { width: 100%; border-collapse: collapse; font-size: var(--md-text-xs); white-space: nowrap; }caption { caption-side: top; text-align: left; padding: var(--md-space-1) var(--md-space-3); }th, td { padding: var(--md-space-2); border-bottom: 1px solid var(--md-border-inactive); text-align: left; }th { position: sticky; top: 0; background: var(--md-bg-header); color: var(--md-fg-subtle); font-weight: 500; }td { font-family: var(--md-font-mono); }
.actions, .footer-actions { display: flex; flex-wrap: wrap; gap: var(--md-space-2); padding-top: var(--md-space-2); }.footer-actions { justify-content: flex-end; }
.action { height: 26px; padding: 0 var(--md-space-2); font-size: var(--md-text-xs); }.action.primary { background: var(--md-accent); border-color: var(--md-accent); color: var(--md-accent-contrast); }
.table-scroll:focus-visible, .log-tail:focus-visible, .failure-detail pre:focus-visible, .crash-card pre:focus-visible { outline: 2px solid var(--md-accent); outline-offset: -2px; }
</style>
