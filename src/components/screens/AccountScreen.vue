<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { LoadState, ProviderId, ServiceAccount } from "../contracts";
import EmptyState from "../common/EmptyState.vue";
import Icon from "../common/Icon.vue";
import ListShell from "../common/ListShell.vue";
import ProviderMark from "../common/ProviderMark.vue";

/**
 * `<AccountScreen>` — the accounts/services screen (docs/04 §S14): the list of
 * connected hosting accounts and the selected account's detail (auth method,
 * organizations, keys, repository count). Connecting an account opens the
 * provider's flow, which the shell owns.
 */
const props = withDefaults(
  defineProps<{
    accounts: LoadState<ServiceAccount[]>;
    selectedId?: string | null;
    /**
     * Non-null when the account/service backend is not built: the "Add account"
     * affordance disables with this reason instead of pretending to work.
     */
    unavailable?: string | null;
  }>(),
  { selectedId: null, unavailable: null },
);

const emit = defineEmits<{
  select: [id: string];
  addAccount: [];
  remove: [id: string];
  connect: [id: string];
  disconnect: [id: string];
  refresh: [];
  copyKey: [keyId: string];
}>();

const list = computed(() => (props.accounts.status === "data" ? props.accounts.data : []));

const selected = computed(() => list.value.find((account) => account.id === props.selectedId) ?? null);

const { t } = useI18n();

/**
 * Vendor names stay inline: a product's own name is not a translatable string,
 * and a catalogue entry whose two languages are identical reads as a
 * forgotten translation. Only the generic `custom` slot is a real word, so it
 * goes through the catalogue — and through a function, not a table, so the
 * group heading relabels itself on a language switch.
 */
const VENDOR_NAME: Partial<Record<ProviderId, string>> = {
  github: "GitHub",
  gitlab: "GitLab",
  bitbucket: "Bitbucket",
  azureDevOps: "Azure DevOps",
  stash: "Bitbucket Server",
  beanstalk: "Beanstalk",
};

function labelOf(provider: ProviderId): string {
  return VENDOR_NAME[provider] ?? t("screens.account.providers.other");
}

const byProvider = computed(() => {
  const groups: { provider: ProviderId; accounts: ServiceAccount[] }[] = [];
  for (const account of list.value) {
    const group = groups.find((entry) => entry.provider === account.provider);
    if (group) group.accounts.push(account);
    else groups.push({ provider: account.provider, accounts: [account] });
  }
  return groups;
});
</script>

<template>
  <div class="accounts">
    <div class="accounts-list">
      <header class="accounts-head">
        <h1 class="accounts-title">{{ $t('screens.account.title') }}</h1>
        <span class="accounts-spacer" />
        <button
          type="button"
          class="accounts-add"
          :disabled="!!unavailable"
          :title="unavailable ?? $t('screens.account.add')"
          @click="emit('addAccount')"
        >
          <Icon name="plus" :size="13" />
          {{ $t('screens.account.add') }}
        </button>
      </header>

      <ListShell
        :status="accounts.status"
        empty-icon="empty.services"
        :empty-title="$t('screens.account.emptyTitle')"
        :empty-caption="unavailable ?? $t('screens.account.emptyCaption')"
        :empty-action-label="unavailable ? undefined : $t('screens.account.add')"
        :error-message="accounts.status === 'error' ? accounts.error.message : null"
        :loading-caption="$t('screens.account.loading')"
        @retry="emit('refresh')"
        @empty-action="emit('addAccount')"
      >
        <div v-for="group in byProvider" :key="group.provider" class="account-group">
          <p class="account-group-title">{{ labelOf(group.provider) }}</p>
          <button
            v-for="account in group.accounts"
            :key="account.id"
            type="button"
            class="account-row"
            :class="{ selected: account.id === selectedId }"
            :aria-current="account.id === selectedId ? 'true' : undefined"
            @click="emit('select', account.id)"
          >
            <ProviderMark :provider="account.provider" :label="account.displayName" :size="20" />
            <span class="account-body">
              <span class="account-name">{{ account.displayName }}</span>
              <span class="account-login">{{ account.login }}</span>
            </span>
            <span v-if="!account.connected" class="account-offline" :title="$t('screens.account.notConnected')">
              <Icon name="exclamationmark.triangle" :size="12" />
            </span>
          </button>
        </div>
      </ListShell>
    </div>

    <div class="accounts-detail">
      <EmptyState
        v-if="!selected"
        icon="empty.services"
        :title="$t('screens.account.noneSelected')"
        :caption="$t('screens.account.noneSelectedCaption')"
      />
      <template v-else>
        <header class="detail-head">
          <ProviderMark :provider="selected.provider" :label="selected.displayName" :size="28" />
          <div class="detail-titles">
            <h2 class="detail-name">{{ selected.displayName }}</h2>
            <p class="detail-sub">
              {{ labelOf(selected.provider) }} · {{ selected.login }} · {{ selected.authMethod }}
            </p>
          </div>
          <span class="accounts-spacer" />
          <button
            v-if="selected.connected"
            type="button"
            class="detail-verb"
            @click="emit('disconnect', selected.id)"
          >
            {{ $t('screens.account.signOut') }}
          </button>
          <button v-else type="button" class="detail-verb primary" @click="emit('connect', selected.id)">
            {{ $t('screens.account.connect') }}
          </button>
        </header>

        <dl class="detail-facts">
          <div class="fact">
            <dt>{{ $t('screens.account.repositories') }}</dt>
            <dd>{{ selected.repositoryCount }}</dd>
          </div>
          <div class="fact">
            <dt>{{ $t('screens.account.organizations') }}</dt>
            <dd>
              <span v-if="selected.organizations.length === 0" class="muted">{{ $t('common.none') }}</span>
              <span v-for="org in selected.organizations" :key="org" class="org-chip">{{ org }}</span>
            </dd>
          </div>
        </dl>

        <section class="detail-keys">
          <h3 class="keys-title">{{ $t('screens.account.keys') }}</h3>
          <p v-if="selected.keys.length === 0" class="keys-empty">
            {{ $t('screens.account.noKeys') }}
          </p>
          <div v-for="key in selected.keys" :key="key.id" class="key-row">
            <Icon name="key" :size="13" class="key-icon" />
            <span class="key-title">{{ key.title }}</span>
            <span v-if="key.fingerprint" class="key-fingerprint mono">{{ key.fingerprint }}</span>
            <span class="accounts-spacer" />
            <button type="button" class="key-copy" :title="$t('screens.account.copyKeyId')" @click="emit('copyKey', key.id)">
              <Icon name="doc.on.doc" :size="12" />
            </button>
          </div>
        </section>

        <footer class="detail-foot">
          <button type="button" class="detail-verb danger" @click="emit('remove', selected.id)">
            <Icon name="trash" :size="13" />
            {{ $t('screens.account.remove') }}
          </button>
        </footer>
      </template>
    </div>
  </div>
</template>

<style scoped>
.accounts {
  display: flex;
  min-height: 0;
  flex: 1;
}

.accounts-list {
  width: 300px;
  flex: none;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--md-border);
  min-height: 0;
}

.accounts-head {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  min-height: var(--md-toolbar-height);
  padding: 0 var(--md-space-3);
  border-bottom: 1px solid var(--md-border);
}

.accounts-title {
  margin: 0;
  font-size: var(--md-text-md);
  font-weight: 600;
}

.accounts-spacer {
  flex: 1;
}

.accounts-add {
  display: inline-flex;
  align-items: center;
  gap: var(--md-space-1);
  font-size: var(--md-text-xs);
}

.account-group {
  display: flex;
  flex-direction: column;
  padding: var(--md-space-2) var(--md-space-2) 0;
}

.account-group-title {
  margin: 0 0 var(--md-space-1);
  padding: 0 var(--md-space-2);
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.account-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  border: none;
  background: transparent;
  border-radius: var(--md-radius-md);
  padding: var(--md-space-2);
  text-align: left;
}

.account-row:hover {
  background: var(--md-bg-hover);
}

.account-row.selected {
  background: var(--md-bg-selected);
}

.account-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.account-name {
  font-size: var(--md-text-sm);
  font-weight: 500;
}

.account-login {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.account-offline {
  color: var(--md-warning);
}

.accounts-detail {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: var(--md-space-4);
  gap: var(--md-space-4);
  overflow-y: auto;
}

.detail-head {
  display: flex;
  align-items: center;
  gap: var(--md-space-3);
}

.detail-titles {
  min-width: 0;
}

.detail-name {
  margin: 0;
  font-size: var(--md-text-lg);
  font-weight: 600;
}

.detail-sub {
  margin: 0;
  font-size: var(--md-text-xs);
  color: var(--md-fg-muted);
}

.detail-verb {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 1px solid var(--md-border-strong);
  background: var(--md-bg-surface);
  border-radius: var(--md-radius-md);
  padding: 2px var(--md-space-2);
  font-size: var(--md-text-sm);
}

.detail-verb.primary {
  background: var(--md-accent);
  border-color: var(--md-accent);
  color: var(--md-accent-contrast);
}

.detail-verb.danger {
  color: var(--md-danger);
}

.detail-verb.danger:hover {
  border-color: var(--md-danger);
  background: var(--md-bg-danger-soft);
}

.detail-facts {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: var(--md-space-2);
  padding: var(--md-space-3);
  background: var(--md-bg-sunken);
  border-radius: var(--md-radius-md);
}

.fact {
  display: flex;
  gap: var(--md-space-3);
  font-size: var(--md-text-sm);
}

.fact dt {
  width: 110px;
  flex: none;
  color: var(--md-fg-subtle);
  font-size: var(--md-text-xs);
  padding-top: 2px;
}

.fact dd {
  margin: 0;
  display: flex;
  flex-wrap: wrap;
  gap: var(--md-space-1);
}

.org-chip {
  background: var(--md-badge-bg);
  color: var(--md-badge-fg);
  border-radius: var(--md-radius-sm);
  padding: 0 var(--md-space-1);
  font-size: var(--md-text-xs);
}

.muted {
  color: var(--md-fg-disabled);
}

.detail-keys {
  display: flex;
  flex-direction: column;
  gap: var(--md-space-1);
}

.keys-title {
  margin: 0;
  font-size: var(--md-text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--md-fg-subtle);
}

.keys-empty {
  margin: 0;
  font-size: var(--md-text-sm);
  color: var(--md-fg-disabled);
}

.key-row {
  display: flex;
  align-items: center;
  gap: var(--md-space-2);
  padding: var(--md-space-1) 0;
  border-bottom: 1px solid var(--md-border-inactive);
  font-size: var(--md-text-sm);
}

.key-icon {
  color: var(--md-fg-subtle);
}

.key-fingerprint {
  font-size: var(--md-text-xs);
  color: var(--md-fg-subtle);
}

.key-copy {
  border: none;
  background: transparent;
  color: var(--md-fg-subtle);
  padding: var(--md-space-1);
  border-radius: var(--md-radius-sm);
}

.key-copy:hover {
  background: var(--md-bg-hover);
  color: var(--md-fg);
}

.detail-foot {
  margin-top: auto;
  padding-top: var(--md-space-3);
  border-top: 1px solid var(--md-border-inactive);
}
</style>
