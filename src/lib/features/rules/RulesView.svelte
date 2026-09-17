<script lang="ts">
  import { catalog } from '../../services';
  import { errorCode } from '../../services/errors';
  import { t } from '../../i18n';
  import Dialog from '../../components/Dialog.svelte';
  import CountryFlag from '../../components/CountryFlag.svelte';
  import MaterialIcon from '../../components/MaterialIcon.svelte';
  import SearchField from '../../components/SearchField.svelte';
  import Dropdown from '../../components/Dropdown.svelte';
  import type { Rule } from '../../services/catalog';
  import discordIcon from '../../../assets/discord.png';
  import yandexIcon from '../../../assets/yandex.png';
  const catalogStore = catalog.state;
  let query = $state('');
  let searching = $state(false);
  let menu = $state(false);
  let selectMode = $state(false);
  let selectingRule = $state<Rule | null>(null);
  let pendingServerId = $state('');
  let {
    onChromeChange,
  }: {
    onChromeChange?: (mode: 'default' | 'selection' | 'picker') => void;
  } = $props();
  const activeSub = $derived(
    $catalogStore.subscriptions.find(
      (s) =>
        s.id ===
        $catalogStore.servers.find(
          (s) => s.id === $catalogStore.selectedServerId,
        )?.subscriptionId,
    ) ?? $catalogStore.subscriptions[0],
  );
  import ServersView from '../servers/ServersView.svelte';
  import searchIcon from '../../../assets/search.svg';
  import reloadIcon from '../../../assets/reload.svg';
  import menuIcon from '../../../assets/menu.svg';
  import addIcon from '../../../assets/add.svg';
  import pencilIcon from '../../../assets/pencil.svg';
  import trashIcon from '../../../assets/trash.svg';
  import shareIcon from '../../../assets/share.svg';
  import checkIcon from '../../../assets/check.svg';
  import chevronIcon from '../../../assets/chevron.svg';
  async function refresh() {
    try {
      if (activeSub) await catalog.refreshSubscription(activeSub.id);
      error = '';
    } catch (e) {
      error = t('errors.' + errorCode(e));
    }
  }
  function pickServer(server: { id: string }) {
    if (!selectingRule) return;
    pendingServerId = server.id;
    try {
      catalog.saveRule({ ...selectingRule, serverId: server.id });
      selectingRule = null;
      pendingServerId = '';
      error = '';
    } catch (e) {
      error = t('errors.' + errorCode(e));
    }
  }
  function confirmServer() {
    if (!selectingRule || !pendingServerId) return;
    pickServer({ id: pendingServerId });
  }

  let editing = $state<Rule | null>(null);
  let deleting = $state(false);
  let selected = $state<string[]>([]);
  let error = $state('');
  let exported = $state('');
  let filtered = $derived(
    $catalogStore.rules.filter((r) =>
      r.name.toLowerCase().includes(query.toLowerCase()),
    ),
  );
  let serverOptions = $derived(
    $catalogStore.servers.map((server) => ({
      value: server.id,
      label: `${t('servers.countries.' + server.countryCode)} · ${$catalogStore.subscriptions.find((subscription) => subscription.id === server.subscriptionId)?.name ?? ''}`,
    })),
  );
  $effect(() => {
    onChromeChange?.(
      selectingRule ? 'picker' : selectMode ? 'selection' : 'default',
    );
  });
  function add() {
    error = '';
    editing = {
      id: crypto.randomUUID(),
      name: '',
      isDomain: true,
      serverId: $catalogStore.selectedServerId ?? '',
      enabled: true,
    };
  }
  function setRuleType(isDomain: boolean) {
    if (editing && editing.isDomain !== isDomain) {
      editing = { ...editing, isDomain, name: '' };
      error = '';
    }
  }
  function save(e: SubmitEvent) {
    e.preventDefault();
    if (!editing) return;
    try {
      catalog.saveRule(editing);
      editing = null;
      error = '';
    } catch (e) {
      error = t('errors.' + errorCode(e));
    }
  }
  function toggle(rule: Rule) {
    try {
      catalog.saveRule({ ...rule, enabled: !rule.enabled });
    } catch (e) {
      error = t('errors.' + errorCode(e));
    }
  }
  function remove() {
    try {
      catalog.removeRules(new Set(selected));
      selected = [];
      deleting = false;
      error = '';
    } catch (e) {
      error = t('errors.' + errorCode(e));
    }
  }
</script>

<section class="rules-view" aria-label={t('rules.title')}>
  {#if selectingRule}
    <div class="server-picker-view">
      <ServersView selectedId={pendingServerId} onSelect={pickServer} compact />
      <div class="picker-bar">
        <button
          class="select-bar-btn back-round"
          aria-label={t('common.close')}
          onclick={() => {
            selectingRule = null;
            pendingServerId = '';
          }}><img src={chevronIcon} alt="" /></button
        >
        <button
          class="select-bar-btn confirm-btn"
          aria-label={t('common.save')}
          onclick={confirmServer}><img src={checkIcon} alt="" /></button
        >
      </div>
    </div>
  {:else}
    {#if selectMode}
      <div class="selection-header">
        <div class="sub-info">
          <span class="sub-title">{activeSub?.name ?? t('rules.title')}</span
          ><span class="status-dot" aria-hidden="true"></span>
        </div>
        <button
          class="select-all"
          role="checkbox"
          aria-checked={selected.length === filtered.length &&
            filtered.length > 0}
          onclick={() =>
            (selected =
              selected.length === filtered.length
                ? []
                : filtered.map((rule) => rule.id))}
        >
          <span>{t('rules.selectAll')}</span>
          <span
            class="select-checkbox"
            class:checked={selected.length === filtered.length &&
              filtered.length > 0}
            >{#if selected.length === filtered.length && filtered.length > 0}<img
                src={checkIcon}
                alt=""
              />{/if}</span
          >
        </button>
      </div>
    {:else}
      <div class="header-card-wrapper">
        <div class="header-row">
          <div class="active-sub-card">
            <div class="sub-info">
              <span class="sub-title"
                >{activeSub?.name ?? t('rules.title')}</span
              ><span class="status-dot" aria-hidden="true"></span>
            </div>
            <div class="card-actions">
              <button
                class="action-btn"
                aria-label={t('common.search')}
                aria-expanded={searching}
                onclick={() => {
                  searching = !searching;
                  if (!searching) query = '';
                }}><img src={searchIcon} alt="" /></button
              >
              <button
                class="action-btn"
                aria-label={t('common.reload')}
                onclick={refresh}><img src={reloadIcon} alt="" /></button
              >
              <button
                class="action-btn"
                class:active-action={menu}
                aria-label={t('common.menu')}
                aria-expanded={menu}
                onclick={() => (menu = !menu)}
                ><img src={menuIcon} alt="" /></button
              >
              <button
                class="action-btn"
                aria-label={t('rules.selectMode')}
                onclick={() => (selectMode = true)}
                ><img src={pencilIcon} alt="" /></button
              >
            </div>
          </div>
          <button
            class="separate-add-btn"
            aria-label={t('common.add')}
            disabled={!$catalogStore.servers.length}
            onclick={add}><img src={addIcon} alt="" /></button
          >
        </div>
        {#if menu}
          <div class="popup-menu">
            <button
              class="menu-item"
              onclick={() => {
                selectMode = true;
                menu = false;
              }}
            >
              <span class="menu-item-icon-circle">
                <img src={pencilIcon} alt="" />
              </span>
              <span>{t('common.edit')}</span>
            </button>
            <button
              class="menu-item"
              onclick={() => {
                menu = false;
                void refresh();
              }}
            >
              <span class="menu-item-icon-circle">
                <img src={reloadIcon} alt="" />
              </span>
              <span>{t('common.reload')}</span>
            </button>
            <button
              class="menu-item"
              disabled={!$catalogStore.rules.length}
              onclick={() => {
                exported = JSON.stringify(
                  { version: 1, demo: true, rules: $catalogStore.rules },
                  null,
                  2,
                );
                menu = false;
              }}
            >
              <span class="menu-item-icon-circle">
                <img src={shareIcon} alt="" />
              </span>
              <span>{t('common.export')}</span>
            </button>
            <button
              class="menu-item danger"
              onclick={() => {
                selectMode = true;
                menu = false;
              }}
            >
              <span class="menu-item-icon-circle">
                <img src={trashIcon} alt="" />
              </span>
              <span>{t('common.delete')}</span>
            </button>
          </div>
        {/if}
      </div>
    {/if}
    {#if searching}<SearchField
        bind:value={query}
        placeholder={t('rules.searchPlaceholder')}
        onClose={() => (searching = false)}
      />{/if}
    {#if error}<p role="alert">{error}</p>{/if}
    <div class="rules-list">
      {#each filtered as rule (rule.id)}
        {@const server = $catalogStore.servers.find(
          (s) => s.id === rule.serverId,
        )}
        {@const country = server
          ? t('servers.countries.' + server.countryCode)
          : '—'}
        <article class="rule-row">
          <div class="rule-left">
            <div
              class="app-icon-wrapper"
              class:domain-icon={!/^(discord|yandex)\.exe$/i.test(rule.name)}
            >
              {#if /^(discord|yandex)\.exe$/i.test(rule.name)}<img
                  class="app-icon"
                  src={/^discord/i.test(rule.name) ? discordIcon : yandexIcon}
                  alt=""
                />{:else}<span class="globe-text" aria-hidden="true"
                  ><MaterialIcon
                    name={rule.isDomain ? 'public' : 'description'}
                    size={20}
                  /></span
                >{/if}
            </div>
            <button
              class="rule-text"
              onclick={() => {
                error = '';
                editing = { ...rule };
              }}
              aria-label={t('rules.edit', { name: rule.name })}
              ><span class="rule-name">{rule.name}</span><span
                class="rule-details">{country}</span
              ></button
            >
          </div>
          <div class="rule-right">
            <button
              class="server-pill"
              onclick={() => {
                selectingRule = rule;
                pendingServerId = rule.serverId;
              }}
              aria-label={rule.name + ': ' + t('servers.title')}
            >
              {#if server}<span class="flag-sm"
                  ><CountryFlag
                    countryCode={server.countryCode}
                    size={20}
                  /></span
                >{/if}<span class="server-pill-name">{country}</span>
            </button>
            {#if selectMode}
              <button
                class="select-checkbox"
                class:checked={selected.includes(rule.id)}
                role="checkbox"
                aria-checked={selected.includes(rule.id)}
                aria-label={t('rules.select', { name: rule.name })}
                onclick={() =>
                  (selected = selected.includes(rule.id)
                    ? selected.filter((id) => id !== rule.id)
                    : [...selected, rule.id])}
                >{#if selected.includes(rule.id)}<img
                    src={checkIcon}
                    alt=""
                  />{/if}</button
              >
            {:else}
              <button
                class="toggle-switch"
                class:enabled={rule.enabled}
                role="switch"
                aria-checked={rule.enabled}
                aria-label={t('rules.toggle', { name: rule.name })}
                onclick={() => toggle(rule)}
                ><span class="toggle-thumb"></span></button
              >
            {/if}
          </div>
        </article>
      {:else}<p>{t('rules.empty')}</p>{/each}
    </div>
    {#if selectMode}
      <div class="select-bar">
        <button
          class="select-bar-btn delete-btn"
          aria-label={t('rules.deleteSelected')}
          disabled={!selected.length}
          onclick={() => (deleting = true)}
          ><img src={trashIcon} alt="" /></button
        >
        <button
          class="select-bar-btn confirm-btn"
          aria-label={t('common.close')}
          onclick={() => {
            selectMode = false;
            selected = [];
          }}><img src={checkIcon} alt="" /></button
        >
      </div>
    {/if}
  {/if}
</section>
{#if editing}
  <Dialog title={t('rules.editTitle')} onClose={() => (editing = null)}>
    <form class="panel-form rule-form" onsubmit={save}>
      <p class="form-intro">{t('rules.addHint')}</p>
      <div class="type-field">
        <span class="field-label">{t('rules.kind')}</span>
        <div class="type-options" role="group" aria-label={t('rules.kind')}>
          <button
            type="button"
            class:chosen={editing.isDomain}
            aria-pressed={editing.isDomain}
            onclick={() => setRuleType(true)}>{t('rules.domain')}</button
          >
          <button
            type="button"
            class:chosen={!editing.isDomain}
            aria-pressed={!editing.isDomain}
            onclick={() => setRuleType(false)}>{t('rules.application')}</button
          >
        </div>
      </div>
      <label
        >{t('common.name')}<input
          bind:value={editing.name}
          aria-label={t('common.name')}
          placeholder={editing.isDomain ? 'example.com' : 'Discord.exe'}
          maxlength="253"
          autocomplete="off"
          spellcheck="false"
          required
        /><span class="field-hint"
          >{editing.isDomain
            ? t('rules.domainHint')
            : t('rules.applicationHint')}</span
        ></label
      >
      <div class="server-field">
        <span class="field-label">{t('servers.title')}</span>
        <Dropdown
          label={t('servers.title')}
          options={serverOptions}
          bind:value={editing.serverId}
        />
      </div>
      {#if error}<p class="form-error" role="alert">{error}</p>{/if}
      <button class="form-submit" type="submit"
        >{$catalogStore.rules.some((rule) => rule.id === editing?.id)
          ? t('common.save')
          : t('rules.addRule')}</button
      >
    </form>
  </Dialog>
{/if}
{#if deleting}<Dialog
    title={t('common.delete')}
    onClose={() => (deleting = false)}
    ><p>{t('rules.deleteConfirm', { count: selected.length })}</p>
    {#if error}<p role="alert">{error}</p>{/if}<button
      class="danger"
      onclick={remove}>{t('common.delete')}</button
    ></Dialog
  >{/if}
{#if exported}<Dialog title={t('common.export')} onClose={() => (exported = '')}
    ><p>{t('demo.export')}</p>
    <textarea
      readonly
      value={exported}
      rows="10"
      aria-label={t('common.export')}></textarea></Dialog
  >{/if}

<style>
  .server-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .type-field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .type-options {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
    padding: 4px;
    border: 1px solid #ffffff1f;
    border-radius: 16px;
    background: #ffffff0a;
  }
  .type-options button {
    min-height: 42px;
    padding: 6px 8px;
    border: 0;
    border-radius: 12px;
    background: transparent;
    color: #ffffffa8;
    font-size: 12px;
  }
  .type-options button.chosen {
    background: #ffffff20;
    color: #fff;
    box-shadow: 0 1px 5px #0004;
  }
  .rules-view {
    display: flex;
    flex-direction: column;
    width: 100%;
    flex: 1;
    gap: 10px;
    overflow: hidden;
    padding-top: 0;
    position: relative;
    min-height: 0;
    height: 100%;
  }
  .server-picker-view {
    display: flex;
    flex-direction: column;
    width: 100%;
    flex: 1;
    gap: 10px;
    overflow: hidden;
  }
  .picker-bar {
    display: flex;
    width: 100%;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
    padding: 0 4px;
  }
  .back-round {
    background: #ffffff14;
  }
  .back-round img {
    width: 18px !important;
    height: 18px !important;
    transform: rotate(90deg);
    filter: brightness(0) invert(1);
  }
  .header-card-wrapper {
    position: relative;
    width: 100%;
    flex-shrink: 0;
  }
  .active-sub-card {
    flex: 1;
    background: #00000038;
    border: 1px solid #ffffff14;
    border-radius: 124px;
    height: 62px;
    padding: 0 10px 0 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    backdrop-filter: blur(14px);
    gap: 8px;
  }
  .header-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }
  .selection-header {
    height: 40px;
    min-height: 40px;
    padding: 0 4px 0 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .selection-header + .rules-list {
    margin-top: 6px;
  }
  .select-all {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0;
    border: 0;
    background: transparent;
    font-size: 12px;
  }
  .sub-info {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .sub-title {
    font-size: 15px;
    font-weight: 500;
    letter-spacing: -0.2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: #3aee29;
    box-shadow: 0 0 8px #3aee29a6;
  }
  .card-actions {
    display: flex;
    gap: 4px;
  }
  .action-btn {
    width: 30px;
    height: 30px;
    min-height: 0;
    border-radius: 50%;
    background: none;
    border: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }
  .action-btn:hover,
  .active-action {
    background: #ffffff20;
  }
  .action-btn img {
    width: 18px;
    height: 18px;
    opacity: 0.75;
    filter: brightness(0) invert(1);
  }
  .separate-add-btn {
    width: 50px;
    height: 50px;
    min-height: 0;
    border-radius: 50%;
    background: #00000038;
    border: 1px solid #ffffff14;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .separate-add-btn img {
    width: 20px;
    height: 20px;
    filter: brightness(0) invert(1);
  }
  .popup-menu {
    position: absolute;
    top: 68px;
    right: 8px;
    width: 197px;
    background: #18181a;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 24px;
    padding: 10px;
    backdrop-filter: blur(28px);
    -webkit-backdrop-filter: blur(28px);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    gap: 4px;
    z-index: 50;
    transform-origin: top right;
    animation: menu-in 180ms var(--ease-spring) both;
  }
  .menu-item {
    height: 40px;
    min-height: 0;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.04);
    border: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px 0 8px;
    font-size: 13.5px;
    font-weight: 500;
    transition: background 0.18s ease;
  }
  .menu-item:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  .menu-item.danger:hover {
    background: rgba(243, 57, 57, 0.2);
    color: #ff8080;
  }
  .menu-item-icon-circle {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: #ffffff;
  }
  .menu-item-icon-circle img {
    width: 15px;
    height: 15px;
    filter: brightness(0) invert(1);
  }
  .rules-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .rule-row {
    min-height: 60px;
    border-radius: 124px;
    padding: 0 14px 0 10px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    gap: 8px;
    animation: row-in 320ms var(--ease-spring) both;
  }
  .rule-row:nth-child(2) {
    animation-delay: 35ms;
  }
  .rule-row:nth-child(3) {
    animation-delay: 70ms;
  }
  .rule-row:hover {
    background: #ffffff0a;
  }
  .rule-left {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .app-icon-wrapper {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    overflow: hidden;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .domain-icon {
    background: #ffffff1a;
    border-radius: 50%;
  }
  .app-icon {
    width: 36px;
    height: 36px;
    object-fit: cover;
    border-radius: 10px;
  }
  .globe-text {
    font-size: 18px;
  }
  .rule-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    background: none;
    border: 0;
    padding: 0;
    text-align: left;
    justify-content: center;
  }
  .rule-name {
    font-size: 14px;
    font-weight: 500;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rule-details {
    font-size: 11px;
    color: #ffffff80;
  }
  .rule-right {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .server-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    min-height: 0;
    background: #00000047;
    border: 1px solid #ffffff1a;
    border-radius: 124px;
    padding: 4px 12px 4px 6px;
  }
  .server-pill:hover {
    background: #00000073;
    border-color: #ffffff38;
  }
  .flag-sm {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .server-pill-name {
    font-size: 12px;
    font-weight: 500;
    color: #ffffffe6;
    white-space: nowrap;
    max-width: 80px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .toggle-switch {
    width: 44px;
    height: 26px;
    min-height: 0;
    border-radius: 13px;
    background: #ffffff26;
    border: 0;
    padding: 0;
    position: relative;
    flex-shrink: 0;
  }
  .toggle-switch.enabled {
    background: #ffffffd9;
  }
  .toggle-thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: rgba(90, 90, 105, 0.95);
    box-shadow: 0 1px 3px #0003;
    transition:
      transform 220ms var(--ease-spring),
      background-color 180ms ease;
  }
  .enabled .toggle-thumb {
    transform: translateX(18px);
    background: #2a2a35;
  }
  .select-checkbox {
    width: 26px;
    height: 26px;
    min-height: 0;
    border-radius: 50%;
    background: transparent;
    border: 2px solid #ffffff40;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    flex-shrink: 0;
  }
  .select-checkbox.checked {
    background: white;
    border-color: white;
  }
  .select-checkbox img {
    width: 16px;
    height: 16px;
  }
  .select-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    padding: 4px 0 0;
    flex-shrink: 0;
  }
  .select-bar-btn {
    width: 50px;
    height: 50px;
    border-radius: 50%;
    border: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }
  .select-bar-btn img {
    width: 22px;
    height: 22px;
  }
  .delete-btn {
    background: #d42a2a;
  }
  .delete-btn img {
    filter: brightness(0) invert(1);
  }
  .confirm-btn {
    background: white;
  }
  .confirm-btn img {
    filter: brightness(0);
  }
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.96);
    }
  }
</style>
