<script lang="ts">
	import settingsIcon from '../../../assets/settings.svg';
	import reloadIcon from '../../../assets/reload.svg';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import Dropdown from '../../components/Dropdown.svelte';
	import SettingRow from '../../components/SettingRow.svelte';
	import ToggleSwitch from '../../components/ToggleSwitch.svelte';
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
		<SettingRow
			label={t('settings.language')}
			description={t('settings.languageDesc')}
			class="language-item"
		>
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
		</SettingRow>

		<!-- Routing Mode -->
		<SettingRow
			label={t('settings.routingMode')}
			description={t('settings.routingDesc')}
		>
			<Dropdown
				label={t('settings.routingMode')}
				options={routingOptions}
				value="proxy"
				disabled
				compact
			/>
		</SettingRow>

		<!-- Kill Switch -->
		<SettingRow
			label={t('settings.killSwitch')}
			description={t('settings.killSwitchDesc')}
		>
			<ToggleSwitch
				mode="checkbox"
				disabled
				ariaLabel={t('settings.killSwitch')}
				bind:checked={killSwitch}
			/>
		</SettingRow>

		<!-- Auto Connect -->
		<SettingRow
			label={t('settings.autoConnect')}
			description={t('settings.autoConnectDesc')}
		>
			<ToggleSwitch
				mode="checkbox"
				disabled
				ariaLabel={t('settings.autoConnect')}
				bind:checked={autoConnect}
			/>
		</SettingRow>

		<!-- Xray Core Status -->
		<SettingRow
			label={t('settings.xrayCore')}
			description={t('settings.xrayDesc')}
		>
			<span class="badge">{t('settings.mockReady')}</span>
		</SettingRow>

		<p class="availability">{t('demo.unavailable')}</p>
		<label class="animation-setting">
			<input
				type="checkbox"
				checked={shaderSettings.enabled}
				onchange={(e) => shaderSettings.setEnabled(e.currentTarget.checked)}
			/>
			{t('settings.animation')}
		</label>

		<!-- Tooltips toggle -->
		<SettingRow
			label={t('settings.tooltips')}
			description={t('settings.tooltipsDesc')}
		>
			<ToggleSwitch
				mode="checkbox"
				ariaLabel={t('settings.tooltips')}
				checked={shaderSettings.tooltipsEnabled}
				onchange={(val) => shaderSettings.setTooltipsEnabled(val)}
			/>
		</SettingRow>

		<!-- Section: Interface -->
		<div class="section-header">
			<span class="section-title">{t('settings.uiSection')}</span>
		</div>

		<!-- UI Scale Slider -->
		<SettingRow
			label={t('settings.uiScale')}
			description={t('settings.uiScaleDesc')}
		>
			<div class="range-control">
				<input
					type="range"
					min="0.75"
					max="1.5"
					step="0.05"
					aria-label={t('settings.uiScale')}
					value={shaderSettings.uiScale}
					oninput={(e) =>
						shaderSettings.setUiScale(
							parseFloat((e.target as HTMLInputElement).value),
						)}
					class="range-slider"
				/>
				<span class="range-val">{Math.round(shaderSettings.uiScale * 100)}%</span>
			</div>
		</SettingRow>


		<!-- Section: Background Shader -->
		<div class="section-header">
			<span class="section-title">{t('settings.shaderTitle')}</span>
			<span class="section-desc">{t('settings.shaderDesc')}</span>
		</div>

		<!-- Shader Preset -->
		<SettingRow
			label={t('settings.shaderPreset')}
			description={t('settings.shaderPresetDesc')}
		>
			<Dropdown
				label={t('settings.shaderPreset')}
				options={presetOptions}
				value={shaderSettings.selectedPreset}
				onChange={(value) => shaderSettings.applyPreset(value)}
				compact
			/>
		</SettingRow>

		<!-- Idle Color A (Dark) -->
		<SettingRow
			label={t('settings.shaderIdleColor')}
			description={t('settings.shaderIdleDesc')}
		>
			<div class="color-picker-control">
				<label
					class="color-swatch"
					style="background-color: {shaderSettings.idleColorA}"
				>
					<input
						type="color"
						aria-label={t('settings.shaderIdleColor')}
						value={shaderSettings.idleColorA}
						oninput={(e) =>
							shaderSettings.setIdleColorA(
								(e.target as HTMLInputElement).value,
							)}
					/>
				</label>
				<span class="color-hex">{shaderSettings.idleColorA.toUpperCase()}</span>
			</div>
		</SettingRow>

		<!-- Idle Color B (Light) -->
		<SettingRow
			label={t('settings.shaderIdleColorB')}
			description={t('settings.shaderIdleColorBDesc')}
		>
			<div class="color-picker-control">
				<label
					class="color-swatch"
					style="background-color: {shaderSettings.idleColorB}"
				>
					<input
						type="color"
						aria-label={t('settings.shaderIdleColorB')}
						value={shaderSettings.idleColorB}
						oninput={(e) =>
							shaderSettings.setIdleColorB(
								(e.target as HTMLInputElement).value,
							)}
					/>
				</label>
				<span class="color-hex">{shaderSettings.idleColorB.toUpperCase()}</span>
			</div>
		</SettingRow>

		<!-- Active Color A -->
		<SettingRow
			label={t('settings.shaderColorA')}
			description={t('settings.shaderColorADesc')}
		>
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
		</SettingRow>

		<!-- Active Color B -->
		<SettingRow
			label={t('settings.shaderColorB')}
			description={t('settings.shaderColorBDesc')}
		>
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
		</SettingRow>

		<!-- Exposure Slider -->
		<SettingRow
			label={t('settings.shaderExposure')}
			description={t('settings.shaderExposureDesc')}
		>
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
		</SettingRow>

		<!-- Reset Button -->
		<div class="reset-row">
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
		padding-bottom: 96px;
		scrollbar-width: none;
		-ms-overflow-style: none;
	}
	.settings-view::-webkit-scrollbar {
		display: none;
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

	.lang-select-wrapper {
		display: flex;
		align-items: center;
		gap: 8px;
		background: rgba(255, 255, 255, 0.06);
		border: none;
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
		border: none;
		position: relative;
		cursor: pointer;
		overflow: hidden;
		box-shadow: 0 2px 10px rgba(0, 0, 0, 0.4);
		transition: transform 0.15s ease;
		display: block;
	}

	.color-swatch:hover {
		transform: scale(1.08);
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
		display: flex;
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
		border: none;
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
	}
</style>
