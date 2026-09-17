<script lang="ts">
  import logoIcon from '../../assets/logo.svg';
  import subscriptionsIcon from '../../assets/subscriptions.svg';
  import serversIcon from '../../assets/servers.svg';
  import addIcon from '../../assets/add.svg';
  import globalIcon from '../../assets/mode-global.svg';
  import rulesIcon from '../../assets/mode-rules.svg';
  import { t } from '../i18n';

  interface Props {
    currentTab: 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules';
    routingMode?: 'global' | 'rules';
    onTabChange: (
      tab: 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules',
    ) => void;
    onModeChange?: (mode: 'global' | 'rules') => void;
    onActionClick?: () => void;
  }

  let {
    currentTab = 'home',
    routingMode = 'global',
    onTabChange,
    onModeChange,
    onActionClick,
  }: Props = $props();

  function handleTabChange(
    tab: 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules',
  ) {
    if (tab === currentTab) return;
    onTabChange(tab);
  }
</script>

<nav class="bottom-nav">
  <div class="nav-pill">
    <!-- Slot 1: Home (MISC Logo) -->
    <button
      class="pill-btn"
      aria-current={currentTab === 'home' ? 'page' : undefined}
      class:active={currentTab === 'home'}
      onclick={() => handleTabChange('home')}
      aria-label={t('nav.home')}
      title={t('nav.home')}
    >
      <span class="btn-surface" class:active-surface={currentTab === 'home'}>
        <img
          src={logoIcon}
          alt="MISC"
          class="tab-icon logo-icon"
          class:active-icon={currentTab === 'home'}
        />
      </span>
    </button>

    <!-- Slot 2: Subscriptions (subscriptions.svg) -->
    <button
      class="pill-btn"
      aria-current={currentTab === 'subscriptions' ? 'page' : undefined}
      class:active={currentTab === 'subscriptions'}
      onclick={() => handleTabChange('subscriptions')}
      aria-label={t('nav.subscriptions')}
      title={t('nav.subscriptions')}
    >
      <span
        class="btn-surface"
        class:active-surface={currentTab === 'subscriptions'}
      >
        <img
          src={subscriptionsIcon}
          alt=""
          class="tab-icon sub-icon"
          class:active-icon={currentTab === 'subscriptions'}
        />
      </span>
    </button>

    <!-- Slot 3: Servers (servers.svg) -->
    <button
      class="pill-btn"
      aria-current={currentTab === 'servers' ? 'page' : undefined}
      class:active={currentTab === 'servers'}
      onclick={() => handleTabChange('servers')}
      aria-label={t('nav.servers')}
      title={t('nav.servers')}
    >
      <span class="btn-surface" class:active-surface={currentTab === 'servers'}>
        <img
          src={serversIcon}
          alt=""
          class="tab-icon srv-icon"
          class:active-icon={currentTab === 'servers'}
        />
      </span>
    </button>

    <!-- Slot 4: Add (add.svg) -->
    <button
      class="pill-btn plus-slot"
      onclick={() => onActionClick?.()}
      aria-label={t('common.add')}
      title={t('common.add')}
    >
      <span class="btn-surface plus-btn-surface">
        <img src={addIcon} alt="" class="tab-icon add-icon" />
      </span>
    </button>
  </div>

  <div class="right-slot">
    {#if currentTab === 'subscriptions'}
      <!-- Round plus button for subscriptions tab -->
      <button
        class="round-action-btn"
        aria-label={t('common.add')}
        title={t('common.add')}
        onclick={() => onActionClick?.()}
      >
        <img src={addIcon} alt="" class="round-plus-img" />
      </button>
    {:else}
      <!-- Mode switcher pill (global/rules) — original SVG icons -->
      <div class="action-pill">
        <button
          class="action-pill-btn"
          class:action-active={routingMode === 'global'}
          aria-pressed={routingMode === 'global'}
          aria-label={t('home.modeGlobal')}
          title={t('home.modeGlobal')}
          onclick={() => onModeChange?.('global')}
        >
          <img
            src={globalIcon}
            alt=""
            class="mode-icon"
            class:mode-active={routingMode === 'global'}
          />
        </button>
        <button
          class="action-pill-btn"
          class:action-active={routingMode === 'rules'}
          aria-pressed={routingMode === 'rules'}
          aria-label={t('home.modeRules')}
          title={t('home.modeRules')}
          onclick={() => onModeChange?.('rules')}
        >
          <img
            src={rulesIcon}
            alt=""
            class="mode-icon"
            class:mode-active={routingMode === 'rules'}
          />
        </button>
      </div>
    {/if}
  </div>
</nav>

<style>
  .bottom-nav {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    max-width: 326px;
    gap: 8px;
    flex-shrink: 0;
    margin-top: auto;
    padding-top: 0;
    align-self: center;
    z-index: 20;
  }

  .nav-pill {
    width: 192px;
    height: 50px;
    border-radius: 124px;
    background: rgba(0, 0, 0, 0.28);
    border: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px 7px;
    gap: 6px;
    box-sizing: border-box;
  }

  .pill-btn {
    width: 38px;
    height: 38px;
    min-width: 38px;
    min-height: 0;
    max-width: 38px;
    max-height: 38px;
    aspect-ratio: 1 / 1;
    border-radius: 50%;
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    box-sizing: border-box;
  }

  .btn-surface {
    width: 38px;
    height: 38px;
    min-width: 38px;
    min-height: 0;
    aspect-ratio: 1 / 1;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.06);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    box-sizing: border-box;
    transition:
      background 0.3s cubic-bezier(0.34, 1.56, 0.64, 1),
      transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1),
      box-shadow 0.25s ease;
  }

  .pill-btn:hover .btn-surface {
    background: rgba(255, 255, 255, 0.13);
    transform: scale(1.06);
  }

  .btn-surface.active-surface {
    background: rgba(255, 255, 255, 0.28);
    transform: scale(1.05);
    box-shadow: 0 2px 14px rgba(0, 0, 0, 0.4);
  }

  /* Icons: dim when inactive, full opacity when active */
  .tab-icon {
    object-fit: contain;
    transition:
      filter 0.2s ease,
      opacity 0.2s ease;
    filter: brightness(0) invert(1);
    opacity: 0.45;
  }

  .tab-icon.active-icon {
    opacity: 1;
  }

  .pill-btn:hover .tab-icon {
    opacity: 0.8;
  }

  .logo-icon {
    width: 20px;
    height: 14px;
  }
  .sub-icon {
    width: 17px;
    height: 17px;
  }
  .srv-icon {
    width: 17px;
    height: 17px;
  }
  .add-icon {
    width: 15px;
    height: 15px;
    opacity: 0.7;
  }

  .plus-btn-surface {
    background: rgba(255, 255, 255, 0.05);
  }
  .plus-slot:hover .plus-btn-surface {
    background: rgba(255, 255, 255, 0.13);
  }
  .plus-slot:hover .add-icon {
    opacity: 1;
  }

  /* Right slot */
  .right-slot {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  /* Round plus button for subscriptions tab */
  .round-action-btn {
    width: 48px;
    height: 48px;
    min-width: 48px;
    min-height: 0;
    max-width: 48px;
    max-height: 48px;
    aspect-ratio: 1 / 1;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.28);
    border: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    padding: 0;
    box-sizing: border-box;
    flex-shrink: 0;
    transition:
      background 0.2s ease,
      transform 0.25s cubic-bezier(0.34, 1.56, 0.64, 1),
      border-color 0.2s ease;
  }

  .round-action-btn:hover {
    background: rgba(255, 255, 255, 0.14);
    border-color: rgba(255, 255, 255, 0.18);
    transform: scale(1.06);
  }

  .round-action-btn:active {
    transform: scale(0.93);
  }

  .round-plus-img {
    width: 20px;
    height: 20px;
    filter: brightness(0) invert(1);
    opacity: 0.9;
  }

  /* Mode switcher pill (global/rules) */
  .action-pill {
    width: 90px;
    height: 50px;
    border-radius: 124px;
    background: rgba(0, 0, 0, 0.28);
    border: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 5px 6px;
    box-sizing: border-box;
  }

  .action-pill-btn {
    width: 36px;
    height: 36px;
    min-width: 36px;
    min-height: 0;
    max-width: 36px;
    max-height: 36px;
    aspect-ratio: 1 / 1;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.07);
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    box-sizing: border-box;
    transition:
      background 0.28s cubic-bezier(0.34, 1.56, 0.64, 1),
      transform 0.28s cubic-bezier(0.34, 1.56, 0.64, 1),
      box-shadow 0.25s ease;
  }

  .action-pill-btn:hover {
    background: rgba(255, 255, 255, 0.15);
    transform: scale(1.06);
  }

  .action-pill-btn:active {
    transform: scale(0.92);
  }

  .action-pill-btn.action-active {
    background: #ffffff;
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.35);
    transform: scale(1);
  }

  .mode-icon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    filter: brightness(0) invert(1);
    opacity: 0.7;
    transition:
      opacity 0.2s ease,
      filter 0.2s ease;
  }

  .action-pill-btn:hover .mode-icon {
    opacity: 1;
  }

  .mode-icon.mode-active {
    filter: brightness(0) invert(0);
    opacity: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    .btn-surface,
    .action-pill-btn,
    .round-action-btn {
      transition-duration: 0ms;
    }
  }
</style>
