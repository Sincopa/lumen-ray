<script lang="ts">
  import type { Rule, Server } from '../../services/catalog';
  import { t } from '../../i18n';
  import CountryFlag from '../../components/CountryFlag.svelte';
  import IconButton from '../../components/IconButton.svelte';
  import chevronIcon from '../../../assets/chevron.svg';
  import checkIcon from '../../../assets/check.svg';

  interface Props {
    rule: Rule;
    server?: Server;
    onSave: (rule: Rule) => void;
    onCancel: () => void;
    onSelectProcess: () => void;
    onSelectServer: () => void;
  }

  let {
    rule,
    server,
    onSave,
    onCancel,
    onSelectProcess,
    onSelectServer,
  }: Props = $props();

  // svelte-ignore state_referenced_locally
  let isDomain = $state(rule.isDomain);
  // svelte-ignore state_referenced_locally
  let name = $state(
    rule.isDomain ? rule.name.replace(/^https?:\/\//i, '') : rule.name,
  );
  // svelte-ignore state_referenced_locally
  let serverId = $state(rule.serverId);
  let error = $state('');

  function cleanDomain(val: string): string {
    return val
      .trim()
      .replace(/^https?:\/\//i, '')
      .replace(/\/.*$/, '');
  }

  function handleSave() {
    error = '';
    const finalName = isDomain ? cleanDomain(name) : name.trim();
    if (!finalName) {
      error = isDomain
        ? t('errors.invalidRule')
        : t('rules.chooseProcessError') || 'Выберите процесс';
      return;
    }
    if (!serverId) {
      error = t('errors.invalidServer');
      return;
    }

    onSave({
      ...rule,
      isDomain,
      name: finalName,
      serverId,
    });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSave();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<section class="rule-editor-view" aria-label={t('rules.editTitle')}>
  <div class="form-container">
    <!-- Section 1: Rule Type (Приложения / Домен) -->
    <div class="field-group">
      <span class="field-label">{t('rules.kind')}</span>
      <div
        class="type-segmented-pill"
        role="tablist"
        aria-label={t('rules.kind')}
      >
        <button
          type="button"
          role="tab"
          class="segment-btn"
          class:active-segment={!isDomain}
          aria-selected={!isDomain}
          onclick={() => {
            isDomain = false;
            rule.isDomain = false;
            error = '';
          }}
        >
          {t('rules.applicationTab') || 'Приложения'}
        </button>
        <button
          type="button"
          role="tab"
          class="segment-btn"
          class:active-segment={isDomain}
          aria-selected={isDomain}
          onclick={() => {
            isDomain = true;
            rule.isDomain = true;
            error = '';
          }}
        >
          {t('rules.domainTab') || 'Домен'}
        </button>
      </div>
    </div>

    <!-- Section 2: Domain or Application Field -->
    {#if isDomain}
      <div class="field-group">
        <label class="field-label" for="domain-input">
          {t('rules.domain')}
        </label>
        <div class="input-pill" class:active-pill={name.length > 0}>
          <span class="domain-prefix">https://</span>
          <input
            id="domain-input"
            class="text-input"
            type="text"
            bind:value={name}
            placeholder="ru.pinterest.com"
            autocomplete="off"
            spellcheck="false"
            required
          />
        </div>
      </div>
    {:else}
      <div class="field-group">
        <span class="field-label">
          {t('rules.applicationField') || 'Приложение'}
        </span>
        <button
          type="button"
          class="selector-pill"
          onclick={() => {
            rule.isDomain = false;
            isDomain = false;
            onSelectProcess();
          }}
          aria-label={t('rules.selectProcess') || 'Выбрать процесс'}
        >
          <span class="selector-value" class:placeholder-val={!name}>
            {name || t('rules.selectProcess') || 'Выбрать процесс'}
          </span>
          <img src={chevronIcon} alt="" class="chevron-right" />
        </button>
      </div>
    {/if}

    <!-- Section 3: Server Selection Field -->
    <div class="field-group">
      <span class="field-label">{t('rules.server') || 'Сервер'}</span>
      <button
        type="button"
        class="selector-pill"
        onclick={onSelectServer}
        aria-label={t('rules.selectServer') || 'Выбрать сервер'}
      >
        <div class="server-info">
          {#if server}
            <CountryFlag countryCode={server.countryCode} size={20} />
            <span class="selector-value">
              {t('servers.countries.' + server.countryCode)}
            </span>
          {:else}
            <span class="selector-value placeholder-val">
              {t('rules.selectServer') || 'Выбрать сервер'}
            </span>
          {/if}
        </div>
        <img src={chevronIcon} alt="" class="chevron-right" />
      </button>
    </div>

    {#if error}
      <p class="error-msg" role="alert">{error}</p>
    {/if}
  </div>

  <div class="action-bar">
    <IconButton
      icon={chevronIcon}
      ariaLabel={t('common.close')}
      size="lg"
      variant="surface"
      class="back-round"
      onclick={onCancel}
    />
    <IconButton
      icon={checkIcon}
      ariaLabel={t('common.save')}
      size="lg"
      variant="primary"
      onclick={handleSave}
    />
  </div>
</section>

<style>
  .rule-editor-view {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    flex: 1;
    min-height: 0;
    justify-content: space-between;
    box-sizing: border-box;
    padding-top: 12px;
    position: relative;
  }

  .form-container {
    display: flex;
    flex-direction: column;
    gap: 24px;
    width: 100%;
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
  }

  .field-label {
    font-size: 14px;
    font-weight: 500;
    color: #ffffff;
    letter-spacing: -0.2px;
  }

  /* Segmented switcher (Приложения / Домен) */
  .type-segmented-pill {
    width: 100%;
    height: 56px;
    border-radius: 24px;
    background: rgba(0, 0, 0, 0.22);
    border: none;
    -webkit-backdrop-filter: blur(14px);
    backdrop-filter: blur(14px);
    padding: 6px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .segment-btn {
    flex: 1;
    height: 100%;
    border-radius: 18px;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition:
      background-color 200ms ease,
      color 200ms ease,
      transform 150ms ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .segment-btn:hover:not(.active-segment) {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.08);
  }

  .segment-btn.active-segment {
    background: #ffffff;
    color: #000000;
    font-weight: 600;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25);
  }

  /* Input and Selector Pills */
  .input-pill,
  .selector-pill {
    width: 100%;
    height: 60px;
    border-radius: 24px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid transparent;
    -webkit-backdrop-filter: blur(14px);
    backdrop-filter: blur(14px);
    box-sizing: border-box;
    padding: 0 20px;
    display: flex;
    align-items: center;
    transition:
      border-color 180ms ease,
      box-shadow 180ms ease,
      background-color 180ms ease;
  }

  .selector-pill {
    cursor: pointer;
    text-align: left;
    justify-content: space-between;
    font: inherit;
    color: inherit;
  }

  .selector-pill:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .input-pill:focus-within {
    border-color: rgba(255, 255, 255, 0.35);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.12);
  }

  .server-info {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .domain-prefix {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.4);
    user-select: none;
    flex-shrink: 0;
  }

  .text-input {
    width: 100%;
    height: 100%;
    background: transparent !important;
    border: none !important;
    outline: none !important;
    box-shadow: none !important;
    border-radius: 0 !important;
    font-size: 14px;
    font-family: inherit;
    color: #ffffff;
    padding: 0 !important;
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
  }

  .text-input::placeholder {
    color: rgba(255, 255, 255, 0.45);
  }

  .selector-value {
    font-size: 14px;
    color: #ffffff;
  }

  .selector-value.placeholder-val {
    color: rgba(255, 255, 255, 0.45);
  }

  .chevron-right {
    width: 18px;
    height: 18px;
    transform: rotate(-90deg);
    filter: brightness(0) invert(1);
    opacity: 0.6;
  }

  .error-msg {
    margin: 0;
    padding: 8px 12px;
    background: rgba(212, 42, 42, 0.2);
    border: none;
    border-radius: 16px;
    color: #ff8e8e;
    font-size: 13px;
    text-align: center;
  }

  .action-bar {
    display: flex;
    width: 100%;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
    padding: 0 4px;
    margin-bottom: 33px;
  }

  :global(.back-round .btn-icon) {
    transform: rotate(90deg) !important;
  }
</style>
