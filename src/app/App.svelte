<script lang="ts">
  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import BackgroundShader from '../lib/components/BackgroundShader.svelte';
  import WindowHeader from '../lib/components/WindowHeader.svelte';
  import BottomNav from '../lib/components/BottomNav.svelte';
  import ConnectionView from '../lib/features/connection/ConnectionView.svelte';
  import SubscriptionsView from '../lib/features/subscriptions/SubscriptionsView.svelte';
  import ServersView from '../lib/features/servers/ServersView.svelte';
  import SettingsView from '../lib/features/settings/SettingsView.svelte';
  import RulesView from '../lib/features/rules/RulesView.svelte';
  import ImportSubscription from '../lib/features/subscriptions/ImportSubscription.svelte';
  import { catalog, connection } from '../lib/services';
  import { errorCode } from '../lib/services/errors';
  import { t } from '../lib/i18n';

  type TabName = 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules';

  const tabOrder: Record<TabName, number> = {
    home: 0,
    subscriptions: 1,
    servers: 2,
    rules: 3,
    settings: 4,
  };

  const connectionStore = connection.state;
  const catalogStore = catalog.state;
  const persistenceError = catalog.persistenceError;

  // Local state
  let currentTab = $state<TabName>('home');
  let prevTab = $state<TabName>('home');
  let importing = $state(false);
  let actionError = $state('');
  let ruleChrome = $state<'default' | 'selection' | 'picker'>('default');

  // Derived state
  let connectionState = $derived($connectionStore.status);
  let info = $derived($connectionStore.info);
  let routingMode = $derived($catalogStore.routingMode);
  let selectedServer = $derived(
    $catalogStore.servers.find(
      (s) => s.id === (info?.serverId ?? $catalogStore.selectedServerId),
    ),
  );
  let selectedCountry = $derived(selectedServer?.countryCode ?? 'fi');

  onMount(() => {
    void connection.initialize();
  });

  // Action handlers
  const toggleConnection = () => {
    connection.toggle($catalogStore.selectedServerId);
  };

  async function chooseServer(server: { id: string }) {
    try {
      await catalog.selectServer(server.id);
      currentTab = 'home';
    } catch (e) {
      actionError = t('errors.' + errorCode(e));
    }
  }

  function changeMode(mode: 'global' | 'rules') {
    try {
      catalog.setRoutingMode(mode);
      if (mode === 'rules') {
        handleTabChange('rules');
      }
    } catch (e) {
      actionError = t('errors.' + errorCode(e));
    }
  }

  function handleTabChange(tab: TabName) {
    prevTab = currentTab;
    currentTab = tab;
    if (tab !== 'rules') {
      ruleChrome = 'default';
    }
  }

  // Direction: +1 = slide right→left (going deeper), -1 = slide left→right (going back)
  function getDir(to: string, from: string) {
    return (tabOrder[to as TabName] ?? 0) > (tabOrder[from as TabName] ?? 0)
      ? 1
      : -1;
  }
</script>

<main class="app-viewport">
  <!-- Native WebGL2 Background Fluid Light Shader -->
  <BackgroundShader active={connectionState !== 'disconnected'} />

  <div class="app-ui" class:dim-backdrop={currentTab !== 'home'}>
    <!-- Top Desktop Window Bar -->
    <WindowHeader
      title="lumen"
      darkSurface={currentTab !== 'home'}
      onOpenSettings={() => {
        currentTab = currentTab === 'settings' ? 'home' : 'settings';
      }}
    />

    <p class="demo-notice sr-only">{t('demo.notice')}</p>

    {#if $persistenceError}
      <p role="alert">{t('errors.storage')}</p>
    {/if}

    {#if actionError}
      <p role="alert">{actionError}</p>
    {/if}

    <!-- Tab Views with directional fly transitions -->
    <div class="view-content">
      {#if currentTab === 'rules'}
        <RulesView onChromeChange={(mode) => (ruleChrome = mode)} />
      {:else}
        {#key currentTab}
          <div
            class="tab-view"
            in:fly={{
              x: getDir(currentTab, prevTab) * 24,
              duration: 260,
              easing: cubicOut,
              opacity: 0,
            }}
          >
            {#if currentTab === 'home'}
              <ConnectionView
                {connectionState}
                {info}
                {selectedCountry}
                server={selectedServer}
                busy={$connectionStore.busy}
                error={$connectionStore.error}
                onRetry={() => connection.initialize()}
                onToggle={toggleConnection}
                onSelectServer={() => (currentTab = 'servers')}
              />
            {:else if currentTab === 'subscriptions'}
              <SubscriptionsView />
            {:else if currentTab === 'servers'}
              <ServersView
                selectedId={$catalogStore.selectedServerId ?? ''}
                disabled={$connectionStore.busy ||
                  connectionState === 'connected'}
                onSelect={chooseServer}
                onManageSubscription={() => handleTabChange('subscriptions')}
              />
            {:else if currentTab === 'settings'}
              <SettingsView />
            {/if}
          </div>
        {/key}
      {/if}
    </div>

    <!-- Bottom Navigation Bar -->
    {#if currentTab !== 'rules' || ruleChrome === 'default'}
      <BottomNav
        {currentTab}
        {routingMode}
        onTabChange={handleTabChange}
        onModeChange={changeMode}
        onActionClick={() => (importing = true)}
      />
    {/if}
  </div>

  {#if importing}
    <ImportSubscription onClose={() => (importing = false)} />
  {/if}
</main>

<style>
  .demo-notice {
    font-size: 12px;
    color: #d6c4ed;
    margin: 0 0 8px;
    text-align: center;
  }
  .app-viewport {
    position: relative;
    width: min(390px, 100vw);
    height: min(845px, 100vh);
    display: flex;
    justify-content: center;
    align-items: center;
    overflow: hidden;
    background-color: #08080b;
    border-radius: var(--radius-window);
    clip-path: inset(0 round var(--radius-window));
  }

  .app-ui {
    position: relative;
    z-index: 10;
    display: flex;
    flex-direction: column;
    width: 100%;
    max-width: 390px;
    min-width: 0;
    height: 100%;
    min-height: 0;
    padding: 16px 16px 33px;
    box-sizing: border-box;
    border-radius: var(--radius-window);
    transition: background-color 0.3s ease;
  }

  .app-ui::before {
    content: '';
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    border-radius: var(--radius-window);
    pointer-events: none;
    z-index: -1;
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    opacity: 0;
    transition: opacity 0.35s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .app-ui.dim-backdrop::before {
    opacity: 1;
  }

  .view-content {
    display: flex;
    flex-direction: column;
    flex: 1;
    width: 100%;
    overflow: hidden;
    position: relative;
  }

  .tab-view {
    display: flex;
    flex-direction: column;
    flex: 1;
    width: 100%;
    overflow: hidden;
    position: absolute;
    inset: 0;
  }
</style>
