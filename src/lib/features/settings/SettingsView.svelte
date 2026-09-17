<script lang="ts">
  import settingsIcon from '../../../assets/settings.svg';
  import reloadIcon from '../../../assets/reload.svg';
  import CountryFlag from '../../components/CountryFlag.svelte';
  import Dropdown from '../../components/Dropdown.svelte';
  import {
    i18n,
    t,
    SUPPORTED_LANGUAGES,
    type SupportedLanguage,
  } from '../../i18n';
  import {
    shaderSettings,
    SHADER_PRESETS,
  } from '../../stores/shaderSettings.svelte';

  let killSwitch = $state(false);
  let autoConnect = $state(false);
  const languageOptions = SUPPORTED_LANGUAGES.map((lang) => ({
    value: lang.code,
    label: lang.name,
  }));
  let routingOptions = $derived([
    { value: 'proxy', label: t('settings.modeProxy') },
    { value: 'tun', label: t('settings.modeTun') },
  ]);
  let presetOptions = $derived([
    ...SHADER_PRESETS.map((preset) => ({
      value: preset.id,
      label: t(preset.nameKey),
    })),
    ...(shaderSettings.selectedPreset === 'custom'
      ? [{ value: 'custom', label: t('settings.shaderPresetCustom') }]
      : []),
  ]);
</script>

<div class="settings-view">
  <div class="header">
    <div class="title-group">
      <img src={settingsIcon} alt="" width="20" height="20" class="icon" />
      <span class="title">{t('settings.title')}</span>
    </div>
  </div>

  <div class="settings-group">
    <!-- Language Switcher -->
    <div class="setting-item language-item">
      <div class="item-text">
        <span class="label">{t('settings.language')}</span>
        <span class="desc">{t('settings.languageDesc')}</span>
      </div>
      <div class="lang-select-wrapper">
        <CountryFlag
          countryCode={i18n.currentLanguage === 'ru' ? 'ru' : 'us'}
          size={20}
        />
        <Dropdown
          label={t('settings.language')}
          options={languageOptions}
          value={i18n.currentLanguage}
          onChange={(value) => i18n.setLanguage(value as SupportedLanguage)}
          compact
        />
      </div>
    </div>

    <!-- Routing Mode -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.routingMode')}</span>
        <span class="desc">{t('settings.routingDesc')}</span>
      </div>
      <Dropdown
        label={t('settings.routingMode')}
        options={routingOptions}
        value="proxy"
        disabled
        compact
      />
    </div>

    <!-- Kill Switch -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.killSwitch')}</span>
        <span class="desc">{t('settings.killSwitchDesc')}</span>
      </div>
      <label class="switch">
        <input
          type="checkbox"
          disabled
          aria-label={t('settings.killSwitch')}
          bind:checked={killSwitch}
        />
        <span class="slider"></span>
      </label>
    </div>

    <!-- Auto Connect -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.autoConnect')}</span>
        <span class="desc">{t('settings.autoConnectDesc')}</span>
      </div>
      <label class="switch">
        <input
          type="checkbox"
          disabled
          aria-label={t('settings.autoConnect')}
          bind:checked={autoConnect}
        />
        <span class="slider"></span>
      </label>
    </div>

    <!-- Xray Core Status -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.xrayCore')}</span>
        <span class="desc">{t('settings.xrayDesc')}</span>
      </div>
      <span class="badge">{t('settings.mockReady')}</span>
    </div>

    <p class="availability">{t('demo.unavailable')}</p>
    <label class="animation-setting"
      ><input
        type="checkbox"
        checked={shaderSettings.enabled}
        onchange={(e) => shaderSettings.setEnabled(e.currentTarget.checked)}
      />{t('settings.animation')}</label
    >
    <!-- Section: Background Shader -->
    <div class="section-header">
      <span class="section-title">{t('settings.shaderTitle')}</span>
      <span class="section-desc">{t('settings.shaderDesc')}</span>
    </div>

    <!-- Shader Preset -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.shaderPreset')}</span>
        <span class="desc">{t('settings.shaderPresetDesc')}</span>
      </div>
      <Dropdown
        label={t('settings.shaderPreset')}
        options={presetOptions}
        value={shaderSettings.selectedPreset}
        onChange={(value) => shaderSettings.applyPreset(value)}
        compact
      />
    </div>

    <!-- Idle Color (Dark) -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.shaderIdleColor')}</span>
        <span class="desc">{t('settings.shaderIdleDesc')}</span>
      </div>
      <div class="color-picker-control">
        <label
          class="color-swatch"
          style="background-color: {shaderSettings.idleColor}"
        >
          <input
            type="color"
            aria-label={t('settings.shaderIdleColor')}
            value={shaderSettings.idleColor}
            oninput={(e) =>
              shaderSettings.setIdleColor((e.target as HTMLInputElement).value)}
          />
        </label>
        <span class="color-hex">{shaderSettings.idleColor.toUpperCase()}</span>
      </div>
    </div>

    <!-- Active Color A -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.shaderColorA')}</span>
        <span class="desc">{t('settings.shaderColorADesc')}</span>
      </div>
      <div class="color-picker-control">
        <label
          class="color-swatch"
          style="background-color: {shaderSettings.colorA}"
        >
          <input
            type="color"
            aria-label={t('settings.shaderColorA')}
            value={shaderSettings.colorA}
            oninput={(e) =>
              shaderSettings.setColorA((e.target as HTMLInputElement).value)}
          />
        </label>
        <span class="color-hex">{shaderSettings.colorA.toUpperCase()}</span>
      </div>
    </div>

    <!-- Active Color B -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.shaderColorB')}</span>
        <span class="desc">{t('settings.shaderColorBDesc')}</span>
      </div>
      <div class="color-picker-control">
        <label
          class="color-swatch"
          style="background-color: {shaderSettings.colorB}"
        >
          <input
            type="color"
            aria-label={t('settings.shaderColorB')}
            value={shaderSettings.colorB}
            oninput={(e) =>
              shaderSettings.setColorB((e.target as HTMLInputElement).value)}
          />
        </label>
        <span class="color-hex">{shaderSettings.colorB.toUpperCase()}</span>
      </div>
    </div>

    <!-- Exposure Slider -->
    <div class="setting-item">
      <div class="item-text">
        <span class="label">{t('settings.shaderExposure')}</span>
        <span class="desc">{t('settings.shaderExposureDesc')}</span>
      </div>
      <div class="range-control">
        <input
          type="range"
          min="0.8"
          max="2.5"
          step="0.05"
          aria-label={t('settings.shaderExposure')}
          value={shaderSettings.exposure}
          oninput={(e) =>
            shaderSettings.setExposure(
              parseFloat((e.target as HTMLInputElement).value),
            )}
          class="range-slider"
        />
        <span class="range-val">{shaderSettings.exposure.toFixed(2)}x</span>
      </div>
    </div>

    <!-- Reset Button -->
    <div class="setting-item reset-row">
      <button class="reset-btn" onclick={() => shaderSettings.reset()}>
        <img
          src={reloadIcon}
          alt=""
          width="14"
          height="14"
          class="reset-icon"
        />
        <span>{t('settings.shaderReset')}</span>
      </button>
    </div>
  </div>
</div>

<style>
  .availability {
    font-size: 13px;
    color: #d4c1e8;
  }
  .animation-setting {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .animation-setting input {
    width: 24px;
  }
  .settings-view {
    display: flex;
    flex-direction: column;
    width: 100%;
    flex: 1;
    gap: 16px;
    overflow-y: auto;
  }

  .header {
    display: flex;
    align-items: center;
    margin-top: 4px;
    margin-bottom: 8px;
  }

  .title-group {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .title {
    font-family: var(--font-family-base);
    font-size: 16px;
    font-weight: 600;
    color: #ffffff;
  }

  .settings-group {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .setting-item {
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 20px;
    padding: 16px 20px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    backdrop-filter: blur(12px);
  }

  .item-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .label {
    font-family: var(--font-family-base);
    font-size: 14px;
    font-weight: 500;
    color: #ffffff;
  }

  .desc {
    font-family: var(--font-family-base);
    font-size: 13px;
    color: #c7bcd0;
  }

  .lang-select-wrapper {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    padding: 4px 8px;
  }

  .badge {
    font-family: var(--font-family-base);
    font-size: 13px;
    color: #86e16f;
    background: rgba(134, 225, 111, 0.15);
    padding: 4px 10px;
    border-radius: 20px;
    font-weight: 500;
  }

  .switch {
    position: relative;
    display: inline-block;
    width: 44px;
    height: 24px;
  }

  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(255, 255, 255, 0.15);
    transition: 0.3s;
    border-radius: 24px;
  }

  .slider:before {
    position: absolute;
    content: '';
    height: 18px;
    width: 18px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    transition: 0.3s;
    border-radius: 50%;
  }

  input:checked + .slider {
    background-color: #6841ee;
  }

  input:checked + .slider:before {
    transform: translateX(20px);
  }

  .section-header {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 14px;
    margin-bottom: 2px;
    padding: 0 4px;
  }

  .section-title {
    font-family: var(--font-family-base);
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.7);
  }

  .section-desc {
    font-family: var(--font-family-base);
    font-size: 13px;
    color: #c7bcd0;
  }

  .color-picker-control {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .color-swatch {
    width: 30px;
    height: 30px;
    border-radius: 10px;
    border: 2px solid rgba(255, 255, 255, 0.25);
    position: relative;
    cursor: pointer;
    overflow: hidden;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.4);
    transition:
      transform 0.15s ease,
      border-color 0.15s ease;
    display: block;
  }

  .color-swatch:hover {
    transform: scale(1.08);
    border-color: rgba(255, 255, 255, 0.6);
  }

  .color-swatch input[type='color'] {
    position: absolute;
    top: -10px;
    left: -10px;
    width: 60px;
    height: 60px;
    opacity: 0;
    cursor: pointer;
    padding: 0;
    margin: 0;
  }

  .color-hex {
    font-family: var(--font-family-base);
    font-size: 12px;
    color: rgba(255, 255, 255, 0.75);
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    min-width: 65px;
  }

  .range-control {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    width: 100%;
  }

  .range-slider {
    flex: 1;
    accent-color: #6841ee;
    cursor: pointer;
    height: 6px;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 3px;
  }

  .range-val {
    font-family: var(--font-family-base);
    font-size: 12px;
    color: rgba(255, 255, 255, 0.85);
    font-variant-numeric: tabular-nums;
    width: 38px;
    text-align: right;
  }

  .reset-row {
    justify-content: flex-end;
    background: transparent;
    border: none;
    padding: 4px 0 16px 0;
  }

  .reset-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    padding: 8px 16px;
    color: rgba(255, 255, 255, 0.85);
    font-family: var(--font-family-base);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .reset-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.25);
  }
</style>
