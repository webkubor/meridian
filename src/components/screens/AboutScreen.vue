<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import type { AppInfo, FeedbackKind, ReleaseNotes, UpdateCheck } from "../../bridges/bindings";
import { updateAvailable } from "../../bridges/bindings";
import type { LoadState } from "../../stores/loadState";
import { formatBytes, formatRelative, LOCALES, type Locale } from "../../i18n";
import type { InstallState } from "../../composables/useApp";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import ProgressTrack from "../common/ProgressTrack.vue";
import ReleaseNotesPanel from "../about/ReleaseNotesPanel.vue";
import SegmentedControl from "../common/SegmentedControl.vue";
import SwitchToggle from "../common/SwitchToggle.vue";

/**
 * `<AboutScreen>` — everything that describes the *application* rather than a
 * repository: which version this is, whether a newer one exists, what changed in
 * each release, and where to report a problem.
 *
 * Dumb by design, like the other screens: state arrives as props, intent leaves
 * as events. Nothing here calls `invoke` or touches the network, which means the
 * whole screen can be rendered in a test with plain objects.
 */
const props = withDefaults(
  defineProps<{
    info: LoadState<AppInfo>;
    check: LoadState<UpdateCheck>;
    notes: LoadState<ReleaseNotes>;
    install: InstallState;
    /** 0–1, or null when the server did not state a total size. */
    installProgress: number | null;
    autoCheck: boolean;
    /** Epoch milliseconds, or null when no check has run in this session. */
    lastCheckedAt: number | null;
    locale: Locale;
    /** True while a feedback URL is being built. */
    feedbackBusy?: boolean;
    /** Set when the last feedback attempt failed. */
    feedbackFailed?: boolean;
  }>(),
  { feedbackBusy: false, feedbackFailed: false },
);

const emit = defineEmits<{
  check: [];
  install: [];
  restart: [];
  copyDiagnostics: [];
  openDiagnostics: [];
  retryNotes: [];
  openLink: [url: string];
  feedback: [kind: FeedbackKind, title: string];
  /**
   * The reader asked for the release notes in a particular language.
   *
   * Separate from `update:locale`: reading the notes in English does not mean
   * wanting the application in English, and the two are remembered apart.
   */
  notesLocale: [locale: Locale];
  "update:autoCheck": [value: boolean];
  "update:locale": [value: Locale];
}>();

const { t } = useI18n();

const app = computed(() => (props.info.status === "data" ? props.info.data : null));

const appName = computed(() => app.value?.name ?? t("app.name"));

/* ---- version line -------------------------------------------------------- */

const versionLine = computed(() => {
  if (!app.value) return "";
  const commit = app.value.commit ? ` (${app.value.commit})` : "";
  return `${app.value.version}${commit}`;
});

const buildProfileLabel = computed(() => {
  if (!app.value) return "";
  return app.value.buildProfile === "debug"
    ? t("about.buildProfileDebug")
    : t("about.buildProfileRelease");
});

const platformLine = computed(() => {
  if (!app.value) return "";
  const os = [app.value.osName, app.value.osVersion].filter(Boolean).join(" ");
  return [os, app.value.arch].filter(Boolean).join(" · ");
});

/* ---- updates ------------------------------------------------------------- */

/**
 * What to say about the update state. Modelled as one exhaustive switch over the
 * union so a new `UpdateCheck` variant in Rust is a TypeScript error here rather
 * than a silent blank panel.
 */
type UpdateSummary =
  | { kind: "loading" }
  | { kind: "failed"; detail: string | null }
  | { kind: "unavailable"; title: string; detail: string }
  | { kind: "current"; version: string }
  | { kind: "available"; version: string; publishedAt: string | null; bytes: number | null };

const updateSummary = computed<UpdateSummary>(() => {
  const state = props.check;

  if (state.status === "loading") return { kind: "loading" };
  if (state.status === "error") {
    return { kind: "failed", detail: state.error.message ?? null };
  }
  if (state.status === "empty") return { kind: "loading" };

  switch (state.data.kind) {
    case "notConfigured":
      return {
        kind: "unavailable",
        title: t("updates.notConfiguredTitle"),
        detail: t("updates.unavailableInThisBuild"),
      };
    case "unsupportedInDebugBuild":
      return {
        kind: "unavailable",
        title: t("updates.debugBuildTitle"),
        detail: t("updates.debugBuildDetail"),
      };
    case "checked": {
      const status = state.data.status;
      if (!updateAvailable(status)) {
        return { kind: "current", version: status.currentVersion };
      }
      return {
        kind: "available",
        version: status.latestVersion ?? status.currentVersion,
        publishedAt: status.publishedAt ?? null,
        bytes: status.downloadBytes ?? null,
      };
    }
  }
});

const lastCheckedLabel = computed(() => {
  if (props.lastCheckedAt === null) return t("updates.neverChecked");
  return t("updates.lastChecked", { when: formatRelative(props.lastCheckedAt) });
});

/* ---- install ------------------------------------------------------------- */

const installing = computed(
  () => props.install.phase === "downloading" || props.install.phase === "installing",
);

const installLabel = computed(() => {
  const state = props.install;
  if (state.phase === "downloading") {
    const percent = props.installProgress;
    return percent === null
      ? t("updates.downloadingUnknown")
      : t("updates.installing", { percent: Math.round(percent * 100) });
  }
  if (state.phase === "downloaded") return t("updates.preparing");
  if (state.phase === "installing") return t("updates.preparing");
  return "";
});

/** Bytes received so far, as a sentence. Localised unit, not "MB". */
const receivedLabel = computed(() => {
  if (props.install.phase !== "downloading" || props.install.received <= 0) return "";
  return t("updates.downloadingReceived", { received: formatBytes(props.install.received) });
});

const canInstall = computed(
  () => updateSummary.value.kind === "available" && app.value?.canSelfUpdate === true,
);

/* ---- feedback ------------------------------------------------------------ */

const feedbackTitle = ref("");

function sendFeedback(kind: FeedbackKind): void {
  emit("feedback", kind, feedbackTitle.value);
}

/* ---- language ------------------------------------------------------------ */

/**
 * `computed`, not a plain array: built once at setup it would keep whatever
 * language was active when the screen first rendered, so switching languages
 * would leave the switcher's own labels behind.
 */
const localeOptions = computed(() =>
  LOCALES.map((locale) => ({
    id: locale,
    label: locale === "zh-CN" ? t("language.zhCN") : t("language.en"),
  })),
);

const projectUrl = computed(() => {
  const slug = app.value?.repoSlug;
  return slug ? `https://github.com/${slug}` : null;
});
</script>

<template>
  <div class="about">
    <EmptyState
      v-if="info.status === 'loading'"
      icon="info.circle"
      :title="t('common.loading')"
      compact
    />

    <EmptyState
      v-else-if="info.status === 'error'"
      icon="exclamationmark.circle"
      :title="t('errors.title')"
      :caption="info.error.message"
      compact
    />

    <div v-else-if="app" class="about-scroll">
      <!-- Identity ------------------------------------------------------- -->
      <header class="identity">
        <div class="identity-mark" aria-hidden="true">
          <Icon name="arrow.up.forward.square" :size="28" />
        </div>
        <div class="identity-text">
          <h1 class="identity-name">{{ appName }}</h1>
          <p class="identity-version">{{ versionLine }}</p>
        </div>
        <span class="identity-chip">{{ buildProfileLabel }}</span>
      </header>

      <p v-if="app.commit === null" class="identity-note">{{ t("about.commitUnknown") }}</p>

      <!-- Updates -------------------------------------------------------- -->
      <section class="card">
        <header class="card-head">
          <Icon name="arrow.down.circle" :size="15" />
          <h2 class="card-title">{{ t("updates.title") }}</h2>
          <span class="card-spacer" />
          <button
            type="button"
            class="action"
            :disabled="updateSummary.kind === 'loading' || updateSummary.kind === 'unavailable'"
            @click="emit('check')"
          >
            {{ updateSummary.kind === "loading" ? t("updates.checking") : t("updates.checkNow") }}
          </button>
        </header>

        <div class="card-body">
          <!-- Loading ---------------------------------------------------- -->
          <p v-if="updateSummary.kind === 'loading'" class="status-line">
            {{ t("updates.checkingHint") }}
          </p>

          <!-- Could not reach ------------------------------------------- -->
          <template v-else-if="updateSummary.kind === 'failed'">
            <p class="status-line strong">{{ t("updates.failed") }}</p>
            <p class="status-line subtle">{{ t("updates.failedDetail") }}</p>
            <p v-if="updateSummary.detail" class="status-line detail">{{ updateSummary.detail }}</p>
          </template>

          <!-- Cannot check at all --------------------------------------- -->
          <template v-else-if="updateSummary.kind === 'unavailable'">
            <p class="status-line strong">{{ updateSummary.title }}</p>
            <p class="status-line subtle">{{ updateSummary.detail }}</p>
          </template>

          <!-- Up to date ------------------------------------------------- -->
          <p v-else-if="updateSummary.kind === 'current'" class="status-line strong">
            {{ t("updates.upToDate", { version: updateSummary.version }) }}
          </p>

          <!-- Newer version --------------------------------------------- -->
          <template v-else>
            <p class="status-line strong">
              {{ t("updates.available", { version: updateSummary.version }) }}
            </p>
            <p v-if="updateSummary.publishedAt" class="status-line subtle">
              {{ t("updates.publishedOn", { date: updateSummary.publishedAt }) }}
            </p>
            <p v-if="updateSummary.bytes" class="status-line subtle">
              {{ t("updates.downloadSize", { size: formatBytes(updateSummary.bytes) }) }}
            </p>

            <!-- Progress ------------------------------------------------- -->
            <div v-if="installing || install.phase === 'downloaded'" class="progress-block">
              <span class="status-line strong">{{ installLabel }}</span>
              <ProgressTrack
                :state="installProgress === null ? 'indeterminate' : 'determinate'"
                :done="installProgress === null ? 0 : Math.round(installProgress * 100)"
                :total="100"
              />
              <span v-if="receivedLabel" class="status-line subtle">{{ receivedLabel }}</span>
            </div>

            <p v-else-if="install.phase === 'readyToRestart'" class="status-line strong">
              {{ t("updates.readyToRestart") }}
            </p>

            <p v-else-if="install.phase === 'failed'" class="status-line strong">
              {{ t("updates.installFailed") }}
            </p>
            <p v-if="install.phase === 'failed'" class="status-line subtle">
              {{ t("updates.installFailedDetail") }}
            </p>

            <div class="button-row">
              <button
                v-if="install.phase === 'readyToRestart'"
                type="button"
                class="action primary"
                @click="emit('restart')"
              >
                {{ t("updates.restartNow") }}
              </button>
              <button
                v-else-if="canInstall"
                type="button"
                class="action primary"
                :disabled="installing"
                @click="emit('install')"
              >
                {{ t("updates.install") }}
              </button>
            </div>
          </template>

          <p class="status-line faint">{{ lastCheckedLabel }}</p>
        </div>

        <!-- The disclosure is not fine print: it is the reason the switch can
             default to on without that being a lie. -->
        <div class="card-foot">
          <div class="row">
            <div class="row-labels">
              <span class="row-label">{{ t("updates.autoCheck") }}</span>
              <span class="row-help">{{ t("updates.autoCheckNote") }}</span>
            </div>
            <SwitchToggle
              :model-value="autoCheck"
              :aria-label="t('updates.autoCheck')"
              @update:model-value="emit('update:autoCheck', $event)"
            />
          </div>
          <details class="disclosure">
            <summary>{{ t("updates.disclosureTitle") }}</summary>
            <p>{{ t("updates.disclosure") }}</p>
          </details>
        </div>
      </section>

      <!-- Release notes --------------------------------------------------- -->
      <section class="card">
        <header class="card-head">
          <Icon name="doc.text" :size="15" />
          <h2 class="card-title">{{ t("changelog.title") }}</h2>
        </header>
        <div class="card-body flush">
          <ReleaseNotesPanel
            :notes="notes"
            :current-version="app.version"
            :notes-locale="notes.status === 'data' ? notes.data.locale : null"
            @retry="emit('retryNotes')"
            @open-link="emit('openLink', $event)"
            @set-locale="emit('notesLocale', $event)"
          />
        </div>
      </section>

      <!-- Feedback -------------------------------------------------------- -->
      <section class="card">
        <header class="card-head">
          <Icon name="questionmark.circle" :size="15" />
          <h2 class="card-title">{{ t("feedback.title") }}</h2>
        </header>

        <div class="card-body">
          <p class="status-line subtle">{{ t("feedback.subtitle") }}</p>

          <label class="field">
            <span class="row-label">{{ t("feedback.summaryLabel") }}</span>
            <input
              v-model="feedbackTitle"
              class="field-input"
              type="text"
              :placeholder="t('feedback.summaryPlaceholder')"
              :aria-label="t('feedback.summaryLabel')"
            />
          </label>

          <div class="feedback-actions">
            <button
              type="button"
              class="action primary"
              :disabled="feedbackBusy"
              @click="sendFeedback('bug')"
            >
              {{ feedbackBusy ? t("feedback.composing") : t("feedback.bug") }}
            </button>
            <button type="button" class="action" :disabled="feedbackBusy" @click="sendFeedback('feature')">
              {{ t("feedback.feature") }}
            </button>
          </div>

          <p class="status-line subtle">{{ t("feedback.bugNote") }}</p>
          <p v-if="feedbackFailed" class="status-line strong">
            {{ t("feedback.failed") }}
          </p>
          <p v-if="feedbackFailed" class="status-line subtle">{{ t("feedback.failedDetail") }}</p>

          <div class="notice">
            <span class="notice-title">{{ t("feedback.privacyTitle") }}</span>
            <p class="notice-body">{{ t("feedback.privacy") }}</p>
            <p class="notice-body warn">{{ t("feedback.warning") }}</p>
          </div>
        </div>
      </section>

      <!-- Platform, project, language ------------------------------------- -->
      <section class="card">
        <header class="card-head">
          <Icon name="info.circle" :size="15" />
          <h2 class="card-title">{{ t("about.title") }}</h2>
        </header>

        <div class="card-body">
          <dl class="facts">
            <dt>{{ t("about.platform") }}</dt>
            <dd>{{ platformLine }}</dd>
            <dt>{{ t("about.runtime") }}</dt>
            <dd>rustc {{ app.rustVersion }}</dd>
            <dt>{{ t("about.license") }}</dt>
            <dd>{{ t("about.licenseValue") }}</dd>
            <dt>{{ t("about.project") }}</dt>
            <dd>
              <button
                v-if="projectUrl"
                type="button"
                class="link-button"
                @click="emit('openLink', projectUrl)"
              >
                {{ app.repoSlug }}
              </button>
              <span v-else class="muted">{{ t("about.projectUnknown") }}</span>
            </dd>
          </dl>

          <div class="row">
            <div class="row-labels">
              <span class="row-label">{{ t("language.label") }}</span>
              <span class="row-help">{{ t("language.note") }}</span>
            </div>
            <SegmentedControl
              :model-value="locale"
              :options="localeOptions"
              :label="t('language.label')"
              @update:model-value="emit('update:locale', $event as Locale)"
            />
          </div>

          <div class="diagnostics">
            <div class="diagnostics-head">
              <span class="row-label">{{ t("about.diagnostics") }}</span>
              <button type="button" class="action" @click="emit('copyDiagnostics')">
                {{ t("about.copyDiagnostics") }}
              </button>
              <button type="button" class="action" @click="emit('openDiagnostics')">
                {{ t("diagnostics.open") }}
              </button>
            </div>
            <p class="row-help">{{ t("about.copyDiagnosticsHint") }}</p>
            <pre class="diagnostics-text">{{ app.diagnostics }}</pre>
          </div>

          <button v-if="projectUrl" type="button" class="action" @click="emit('openLink', projectUrl)">
            {{ t("about.openProjectPage") }}
          </button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.about {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
}

.about-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--md-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--md-space-4);
  max-width: 760px;
}

/* ---- identity ------------------------------------------------------------ */

.identity {
  display: flex;
  align-items: center;
  gap: var(--md-space-3);
}

.identity-mark {
  flex: none;
  width: 44px;
  height: 44px;
  display: grid;
  place-items: center;
  border-radius: var(--md-radius-md);
  background: var(--md-bg-sunken);
  color: var(--md-accent);
  border: 1px solid var(--md-border);
}

.identity-text {
  flex: 1;
  min-width: 0;
}

.identity-name {
  margin: 0;
  font-size: var(--md-text-xl);
  font-weight: 600;
}

.identity-version {
  margin: 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg-subtle);
  font-variant-numeric: tabular-nums;
}

.identity-chip {
  flex: none;
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--md-fg-subtle);
  background: var(--md-badge-bg);
  border-radius: var(--md-radius-sm);
  padding: 1px 5px;
}

.identity-note {
  margin: calc(-1 * var(--md-space-2)) 0 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
}

/* ---- cards --------------------------------------------------------------- */

.card {
  border: 1px solid var(--md-border);
  border-radius: var(--md-radius-md);
  background: var(--md-bg-surface);
  overflow: hidden;
}

.card-head {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-2) var(--md-space-3);
  border-bottom: 1px solid var(--md-border-inactive);
  color: var(--md-fg-muted);
}

.card-title {
  margin: 0;
  font-size: var(--md-text-sm);
  font-weight: 600;
  color: var(--md-fg);
}

.card-spacer {
  flex: 1;
}

.card-body {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
  padding: var(--md-space-3);
}

.card-body.flush {
  padding: var(--md-space-3);
}

.card-foot {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
  padding: var(--md-space-3);
  border-top: 1px solid var(--md-border-inactive);
  background: var(--md-bg-header);
}

/* ---- text ---------------------------------------------------------------- */

.status-line {
  margin: 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg-muted);
}

.status-line.strong {
  color: var(--md-fg);
  font-weight: 500;
}

.status-line.subtle {
  color: var(--md-fg-subtle);
  font-size: var(--md-text-xs);
}

.status-line.faint {
  color: var(--md-fg-disabled);
  font-size: var(--md-text-xs);
}

.status-line.detail {
  font-family: var(--md-font-mono);
  font-size: var(--md-text-xs);
  color: var(--md-fg-disabled);
  word-break: break-word;
}

/* ---- controls ------------------------------------------------------------ */

.action {
  height: 24px;
  padding: 0 var(--md-space-2);
  font-size: var(--md-text-sm);
}

.action.primary {
  background: var(--md-accent);
  border-color: var(--md-accent);
  color: #fff;
}

.action.primary:hover:not(:disabled) {
  background: var(--md-accent-strong);
  border-color: var(--md-accent-strong);
}

.button-row,
.feedback-actions {
  display: flex;
  gap: var(--md-space-2);
  margin-top: var(--md-space-1);
}

.progress-block {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
}

.row {
  display: flex;
  align-items: center;
  gap: var(--md-space-4);
}

.row-labels {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.row-label {
  font-size: var(--md-text-sm);
}

.row-help {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.field-input {
  height: 26px;
  font-size: var(--md-text-sm);
}

/* ---- disclosure ---------------------------------------------------------- */

.disclosure summary {
  cursor: pointer;
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.disclosure p {
  margin: var(--md-space-1) 0 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  max-width: 68ch;
  line-height: 1.5;
}

/* ---- notice -------------------------------------------------------------- */

.notice {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--md-space-2);
  border: 1px solid var(--md-border-inactive);
  border-radius: var(--md-radius-sm);
  background: var(--md-bg-sunken);
}

.notice-title {
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.notice-body {
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
  max-width: 68ch;
  line-height: 1.5;
}

.notice-body.warn {
  color: var(--md-sun-strong);
}

/* ---- facts --------------------------------------------------------------- */

.facts {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 4px var(--md-space-3);
  margin: 0;
  font-size: var(--md-text-sm);
}

.facts dt {
  color: var(--md-fg-subtle);
}

.facts dd {
  margin: 0;
  min-width: 0;
  word-break: break-word;
}

.link-button {
  border: none;
  background: transparent;
  padding: 0;
  color: var(--md-accent);
  font-size: inherit;
  cursor: pointer;
}

.link-button:hover {
  background: transparent;
  text-decoration: underline;
}

.muted {
  color: var(--md-fg-disabled);
}

/* ---- diagnostics --------------------------------------------------------- */

.diagnostics {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
  margin-top: var(--md-space-1);
}

.diagnostics-head {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
}

.diagnostics-head .row-label {
  flex: 1;
}

.diagnostics-text {
  margin: 0;
  padding: var(--md-space-2);
  border-radius: var(--md-radius-sm);
  background: var(--md-bg-sunken);
  color: var(--md-fg-muted);
  font-family: var(--md-font-mono);
  font-size: var(--md-text-xs);
  white-space: pre-wrap;
  word-break: break-all;
  user-select: text;
}
</style>
