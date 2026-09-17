<script lang="ts">
  import CountryFlag from '../../components/CountryFlag.svelte';
  import SearchField from '../../components/SearchField.svelte';
  import NumberFlow from '@number-flow/svelte';
  import { catalog } from '../../services';
  import { errorCode } from '../../services/errors';
  import { t } from '../../i18n';
  import type { Server } from '../../services/catalog';
  import reloadIcon from '../../../assets/reload.svg';
  import searchIcon from '../../../assets/search.svg';
  import menuIcon from '../../../assets/menu.svg';
  import pencilIcon from '../../../assets/pencil.svg';
  import trashIcon from '../../../assets/trash.svg';
  import shareIcon from '../../../assets/share.svg';

  const catalogStore = catalog.state;
  let {
    selectedId = '',
    disabled = false,
    compact = false,
    onSelect,
    onManageSubscription,
  }: {
    selectedId?: string;
    disabled?: boolean;
    compact?: boolean;
    onManageSubscription?: () => void;
    onSelect?: (server: {
      id: string;
      countryCode: string;
      name: string;
    }) => void;
  } = $props();

  let query = $state('');
  let searching = $state(false);
  let menu = $state(false);
  let busy = $state(false);
  let error = $state('');
  let checked = $state(false);
  let filtered = $derived(
    $catalogStore.servers.filter((server) =>
      (
        t('servers.countries.' + server.countryCode) +
        ' ' +
        server.protocol +
        ' ' +
        ($catalogStore.subscriptions.find(
          (sub) => sub.id === server.subscriptionId,
        )?.name ?? '')
      )
        .toLowerCase()
        .includes(query.toLowerCase()),
    ),
  );
  let activeSubscription = $derived(
    $catalogStore.subscriptions.find(
      (sub) =>
        sub.id ===
        $catalogStore.servers.find(
          (server) =>
            server.id === (selectedId || $catalogStore.selectedServerId),
        )?.subscriptionId,
    ) ?? $catalogStore.subscriptions[0],
  );

  async function refresh() {
    if (busy) return;
    busy = true;
    error = '';
    try {
      await Promise.all(
        $catalogStore.servers.map((server) => catalog.pingServer(server.id)),
      );
      checked = true;
    } catch (e) {
      error = t('errors.' + errorCode(e));
    } finally {
      busy = false;
    }
  }

  function select(server: Server) {
    onSelect?.({
      id: server.id,
      countryCode: server.countryCode,
      name: t('servers.countries.' + server.countryCode),
    });
  }
</script>

<section class="servers-view" class:compact aria-label={t('servers.title')}>
  <div class="header-wrapper">
    <header class="active-sub-card">
      <div class="subscription-title">
        <span>{activeSubscription?.name ?? t('servers.title')}</span>
        <span class="status-dot" aria-hidden="true"></span>
      </div>
      <div class="header-actions">
        <button
          class="header-action"
          aria-label={t('common.search')}
          aria-expanded={searching}
          onclick={() => {
            searching = !searching;
            if (!searching) query = '';
          }}><img src={searchIcon} alt="" /></button
        >
        <button
          class="header-action"
          onclick={refresh}
          disabled={busy || !$catalogStore.servers.length}
          aria-label={t('servers.reloadServers')}
          ><img src={reloadIcon} alt="" /></button
        >
        {#if !compact}<button
            class="header-action"
            class:active={menu}
            aria-label={t('common.menu')}
            aria-expanded={menu}
            onclick={() => (menu = !menu)}><img src={menuIcon} alt="" /></button
          >{/if}
      </div>
    </header>
    {#if menu}
      <div class="popup-menu">
        <button
          onclick={() => {
            menu = false;
            onManageSubscription?.();
          }}
        >
          <span class="menu-item-icon-circle">
            <img src={pencilIcon} alt="" />
          </span>
          <span>{t('common.edit')}</span>
        </button>
        <button
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
        <button disabled>
          <span class="menu-item-icon-circle">
            <img src={shareIcon} alt="" />
          </span>
          <span>{t('common.share')}</span>
        </button>
        <button class="danger" disabled>
          <span class="menu-item-icon-circle">
            <img src={trashIcon} alt="" />
          </span>
          <span>{t('common.delete')}</span>
        </button>
      </div>
    {/if}
  </div>

  {#if searching}<SearchField
      bind:value={query}
      placeholder={t('servers.searchPlaceholder')}
      onClose={() => (searching = false)}
    />{/if}
  {#if disabled}<p class="hint">{t('servers.disconnectFirst')}</p>{/if}
  {#if busy}<p role="status">{t('common.loading')}</p>{:else if checked}<p
      role="status"
      class="hint"
    >
      {t('demo.ping')}
    </p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}

  <div class="servers-list">
    {#each filtered as server (server.id)}
      <button
        class="server-row"
        class:active-server={server.id === selectedId}
        {disabled}
        aria-pressed={server.id === selectedId}
        onclick={() => select(server)}
      >
        <CountryFlag countryCode={server.countryCode} size={32} />
        <span class="server-text"
          ><span class="protocol">{server.protocol}</span><span
            >{t('servers.countries.' + server.countryCode)}</span
          ></span
        >
        <span class="ping"
          >{#if server.id === selectedId}<span
              class="ping-dot"
              aria-hidden="true"
            ></span>{/if}{#if typeof server.latency === 'number'}<NumberFlow
              value={server.latency}
            /> ms{:else if server.latency === 'timeout'}{t(
              'servers.timeout',
            )}{:else}{server.latency} ms{/if}</span
        >
      </button>
    {:else}<p class="hint">{t('servers.empty')}</p>{/each}
  </div>
</section>

<style>
  .servers-view {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100%;
    min-height: 0;
    position: relative;
  }
  .header-wrapper {
    position: relative;
    flex-shrink: 0;
  }
  .active-sub-card {
    height: 62px;
    padding: 0 12px 0 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: #00000038;
    border: 1px solid #ffffff14;
    border-radius: 124px;
    backdrop-filter: blur(14px);
  }
  .subscription-title {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    font-size: 15px;
    font-weight: 500;
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #3aee29;
    box-shadow: 0 0 8px #3aee29a6;
    flex-shrink: 0;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .header-action {
    width: 30px;
    height: 30px;
    min-height: 0;
    border: 0;
    border-radius: 50%;
    padding: 0;
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .header-action:hover,
  .header-action.active {
    background: #ffffff18;
  }
  .header-action img {
    width: 18px;
    height: 18px;
    filter: brightness(0) invert(1);
    opacity: 0.72;
  }
  .popup-menu {
    position: absolute;
    top: 68px;
    right: 8px;
    width: 197px;
    padding: 10px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 24px;
    background: #18181a;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(28px);
    -webkit-backdrop-filter: blur(28px);
    z-index: 30;
    transform-origin: top right;
    animation: menu-in 180ms var(--ease-spring) both;
  }
  .popup-menu button {
    width: 100%;
    height: 40px;
    min-height: 0;
    padding: 0 10px 0 8px;
    border: 0;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.04);
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    text-align: left;
    transition: background 0.18s ease;
  }
  .popup-menu button:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  .popup-menu button.danger:hover {
    background: rgba(243, 57, 57, 0.2);
    color: #ff8080;
  }
  .popup-menu button .menu-item-icon-circle {
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
  .servers-list {
    overflow: auto;
    min-height: 0;
    scrollbar-gutter: stable;
  }
  .server-row {
    width: 100%;
    min-height: 62px;
    display: flex;
    gap: 12px;
    align-items: center;
    border: 1px solid transparent;
    border-radius: 30px;
    background: transparent;
    padding: 10px 12px;
    text-align: left;
    transition:
      background-color 180ms ease,
      transform 180ms var(--ease-spring);
    animation: row-in 320ms var(--ease-spring) both;
  }
  .server-row:nth-child(2) {
    animation-delay: 28ms;
  }
  .server-row:nth-child(3) {
    animation-delay: 56ms;
  }
  .server-row:nth-child(4) {
    animation-delay: 84ms;
  }
  .server-row:nth-child(n + 5) {
    animation-delay: 112ms;
  }
  .server-row:hover,
  .active-server {
    background: #00000026;
    border-color: transparent;
  }
  .server-row:active {
    transform: scale(0.985);
  }
  .server-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
    overflow-wrap: anywhere;
    font-size: 14px;
  }
  .protocol {
    font-size: 10px;
    line-height: 1;
    color: #ffffff70;
    text-transform: uppercase;
  }
  .ping {
    font-size: 13px;
    white-space: nowrap;
    color: #ffffffe6;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .ping-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #3aee29;
    box-shadow: 0 0 8px #3aee29a6;
  }
  .hint {
    font-size: 12px;
    color: #d0c4dc;
    margin: 4px 0;
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
