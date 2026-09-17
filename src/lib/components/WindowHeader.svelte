<script lang="ts">
  import minimizeIcon from '../../assets/minimaze.svg';
  import maximizeIcon from '../../assets/maximaze.svg';
  import crossIcon from '../../assets/cross.svg';
  import { t } from '../i18n';

  import { runtime } from '../runtime';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  let failure = $state(false);
  async function control(action: 'minimize' | 'toggleMaximize' | 'close') {
    if (!runtime.isTauri) return;
    try {
      await getCurrentWindow()[action]();
    } catch {
      failure = true;
    }
  }
  interface Props {
    title?: string;
    showControls?: boolean;
    darkSurface?: boolean;
    onOpenSettings?: () => void;
  }

  let {
    title = 'misc',
    showControls = true,
    darkSurface = false,
    onOpenSettings,
  }: Props = $props();
</script>

<header
  class="window-header"
  class:dark-surface={darkSurface}
  data-tauri-drag-region
>
  <button
    class="logo-link-btn"
    onclick={onOpenSettings}
    title={t('settings.title')}
    aria-label={t('settings.title')}
  >
    <div class="logo">{title}</div>
  </button>
  {#if showControls}
    <div class="controls-pill">
      <button
        class="control-btn"
        onclick={() => control('minimize')}
        aria-label={t('common.minimize')}
        title={t('common.minimize')}
      >
        <img src={minimizeIcon} alt="" width="14" height="14" />
      </button>
      <button
        class="control-btn"
        onclick={() => control('toggleMaximize')}
        aria-label={t('common.maximize')}
        title={t('common.maximize')}
      >
        <img src={maximizeIcon} alt="" width="14" height="14" />
      </button>
      <button
        class="control-btn close"
        onclick={() => control('close')}
        aria-label={t('common.close')}
        title={t('common.close')}
      >
        <img src={crossIcon} alt="" width="14" height="14" />
      </button>
    </div>
  {/if}
</header>
{#if failure}<p role="alert">{t('errors.unavailable')}</p>{/if}

<style>
  .window-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    height: 40px;
    margin-bottom: 16px;
    flex-shrink: 0;
  }

  .logo-link-btn {
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    display: flex;
    align-items: center;
    text-align: left;
    margin-left: 8px;
  }

  .logo {
    font-family: var(--font-family-base);
    font-weight: 700;
    font-size: 24px;
    color: #66666a;
    letter-spacing: -0.5px;
    transition: opacity 0.2s ease;
  }

  .dark-surface .logo {
    color: rgba(255, 255, 255, 0.56);
  }

  .logo-link-btn:hover .logo {
    opacity: 0.8;
  }

  .controls-pill {
    display: flex;
    align-items: center;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 123px;
    width: 112px;
    height: 40px;
    padding: 6px;
    gap: 0;
    justify-content: space-between;
    box-sizing: border-box;
    backdrop-filter: blur(10px);
  }

  .control-btn {
    width: 28px;
    height: 28px;
    min-width: 28px;
    min-height: 0;
    max-width: 28px;
    max-height: 28px;
    aspect-ratio: 1 / 1;
    flex-shrink: 0;
    border-radius: 50%;
    background: transparent;
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    padding: 0;
    box-sizing: border-box;
    transition:
      background 0.2s ease,
      transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .control-btn:hover {
    background: rgba(255, 255, 255, 0.15);
    transform: scale(1.1);
  }

  .control-btn:active {
    transform: scale(0.92);
  }

  .control-btn.close:hover {
    background: rgba(243, 57, 57, 0.6);
  }
</style>
