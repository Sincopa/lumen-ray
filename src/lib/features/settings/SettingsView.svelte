<script lang="ts">
	import chevronIcon from '../../../assets/chevron.svg';
	import reloadIcon from '../../../assets/reload.svg';
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
	let currentSubView = $state<'main' | 'presets' | 'colors'>('main');

	let uiScaleText = $state(Math.round(shaderSettings.uiScale * 100).toString());
	let exposureText = $state(shaderSettings.exposure.toFixed(2));
	let isEditingUiScale = $state(false);
	let isEditingExposure = $state(false);

	$effect(() => {
		if (!isEditingUiScale) {
			uiScaleText = Math.round(shaderSettings.uiScale * 100).toString();
		}
	});

	$effect(() => {
		if (!isEditingExposure) {
			exposureText = shaderSettings.exposure.toFixed(2);
		}
	});

	function commitUiScale() {
		isEditingUiScale = false;
		const parsed = parseInt(uiScaleText, 10);
		if (isNaN(parsed)) {
			uiScaleText = Math.round(shaderSettings.uiScale * 100).toString();
			return;
		}
		const clamped = Math.min(150, Math.max(75, parsed));
		shaderSettings.setUiScale(clamped / 100);
		uiScaleText = clamped.toString();
	}

	function commitExposure() {
		isEditingExposure = false;
		const parsed = parseFloat(exposureText.replace(',', '.'));
		if (isNaN(parsed)) {
			exposureText = shaderSettings.exposure.toFixed(2);
			return;
		}
		const clamped = Math.min(2.5, Math.max(0.8, parsed));
		shaderSettings.setExposure(parseFloat(clamped.toFixed(2)));
		exposureText = clamped.toFixed(2);
	}

	const languageOptions = SUPPORTED_LANGUAGES.map((lang) => ({
		value: lang.code,
		label: lang.name,
	}));

	let routingOptions = $derived([
		{ value: 'proxy', label: t('settings.modeProxy') },
		{ value: 'tun', label: t('settings.modeTun') },
	]);
</script>

<div class="settings-view">
	{#if currentSubView === 'main'}
		<!-- Section 1: General Settings -->
		<div class="settings-section">
			<div class="section-header">
				<span class="section-title">{t('settings.generalSection')}</span>
			</div>
			<div class="settings-group">
				<!-- Language Switcher -->
				<SettingRow label={t('settings.language')}>
					<Dropdown
						label={t('settings.language')}
						options={languageOptions}
						value={i18n.currentLanguage}
						onChange={(value) => i18n.setLanguage(value as SupportedLanguage)}
						compact
					/>
				</SettingRow>

				<!-- Animated Background -->
				<SettingRow label={t('settings.animation')}>
					<ToggleSwitch
						mode="checkbox"
						ariaLabel={t('settings.animation')}
						checked={shaderSettings.enabled}
						onchange={(val) => shaderSettings.setEnabled(val)}
					/>
				</SettingRow>

				<!-- Tooltips toggle -->
				<SettingRow label={t('settings.tooltips')}>
					<ToggleSwitch
						mode="checkbox"
						ariaLabel={t('settings.tooltips')}
						checked={shaderSettings.tooltipsEnabled}
						onchange={(val) => shaderSettings.setTooltipsEnabled(val)}
					/>
				</SettingRow>

				<!-- UI Scale Slider & Input -->
				<SettingRow label={t('settings.uiScale')}>
					<div class="range-control">
						<input
							type="range"
							min="0.75"
							max="1.5"
							step="0.05"
							aria-label={t('settings.uiScale')}
							value={shaderSettings.uiScale}
							oninput={(e) => {
								const val = parseFloat((e.target as HTMLInputElement).value);
								shaderSettings.setUiScale(val);
								if (!isEditingUiScale) {
									uiScaleText = Math.round(val * 100).toString();
								}
							}}
							class="range-slider"
						/>
						<div class="range-input-wrapper">
							<input
								type="text"
								inputmode="numeric"
								pattern="[0-9]*"
								class="range-text-input"
								aria-label="{t('settings.uiScale')} (%)"
								bind:value={uiScaleText}
								onfocus={() => (isEditingUiScale = true)}
								onblur={commitUiScale}
								onkeydown={(e) => {
									if (e.key === 'Enter') {
										(e.target as HTMLInputElement).blur();
									}
								}}
							/>
							<span class="range-unit">%</span>
						</div>
					</div>
				</SettingRow>
			</div>
		</div>

		<!-- Section 2: Network / Core Settings -->
		<div class="settings-section">
			<div class="section-header">
				<span class="section-title">{t('settings.networkSection')}</span>
			</div>
			<div class="settings-group">
				<!-- Routing Mode -->
				<SettingRow label={t('settings.routingMode')}>
					<Dropdown
						label={t('settings.routingMode')}
						options={routingOptions}
						value="proxy"
						disabled
						compact
					/>
				</SettingRow>

				<!-- Kill Switch -->
				<SettingRow label={t('settings.killSwitch')}>
					<ToggleSwitch
						mode="checkbox"
						disabled
						ariaLabel={t('settings.killSwitch')}
						bind:checked={killSwitch}
					/>
				</SettingRow>

				<!-- Auto Connect -->
				<SettingRow label={t('settings.autoConnect')}>
					<ToggleSwitch
						mode="checkbox"
						disabled
						ariaLabel={t('settings.autoConnect')}
						bind:checked={autoConnect}
					/>
				</SettingRow>

				<!-- Xray Core Status -->
				<SettingRow label={t('settings.xrayCore')}>
					<span class="badge">{t('settings.mockReady')}</span>
				</SettingRow>
			</div>
			<p class="availability">{t('demo.unavailable')}</p>
		</div>

		<!-- Section 3: Background Shader -->
		<div class="settings-section">
			<div class="section-header">
				<span class="section-title">{t('settings.shaderTitle')}</span>
			</div>
			<div class="settings-group">
				<!-- Presets -->
				<SettingRow
					label={t('settings.presets')}
					showChevron
					onclick={() => (currentSubView = 'presets')}
				/>

				<!-- Custom Colors -->
				<SettingRow
					label={t('settings.customColors')}
					showChevron
					onclick={() => (currentSubView = 'colors')}
				/>
			</div>
		</div>
	{:else if currentSubView === 'presets'}
		<!-- Presets Subview -->
		<div class="subview-container">
			<div class="subview-header">
				<button
					type="button"
					class="back-btn"
					onclick={() => (currentSubView = 'main')}
					aria-label={t('common.back') || 'Назад'}
				>
					<img
						src={chevronIcon}
						alt=""
						class="back-chevron"
						aria-hidden="true"
					/>
					<span class="subview-title">{t('settings.presets')}</span>
				</button>
			</div>

			<div class="settings-group">
				{#each SHADER_PRESETS as preset (preset.id)}
					<button
						type="button"
						class="preset-item"
						class:selected={shaderSettings.selectedPreset === preset.id}
						onclick={() => shaderSettings.applyPreset(preset.id)}
					>
						<div class="preset-left">
							<div class="swatches-pair">
								<span
									class="swatch-dot"
									style="background-color: {preset.colorA}"
								></span>
								<span
									class="swatch-dot swatch-overlap"
									style="background-color: {preset.colorB}"
								></span>
							</div>
							<span class="preset-label">{t(preset.nameKey)}</span>
						</div>
						{#if shaderSettings.selectedPreset === preset.id}
							<span class="check-mark" aria-hidden="true">✓</span>
						{/if}
					</button>
				{/each}

				{#if shaderSettings.selectedPreset === 'custom'}
					<div class="preset-item selected">
						<div class="preset-left">
							<div class="swatches-pair">
								<span
									class="swatch-dot"
									style="background-color: {shaderSettings.colorA}"
								></span>
								<span
									class="swatch-dot swatch-overlap"
									style="background-color: {shaderSettings.colorB}"
								></span>
							</div>
							<span class="preset-label">
								{t('settings.shaderPresetCustom')}
							</span>
						</div>
						<span class="check-mark" aria-hidden="true">✓</span>
					</div>
				{/if}
			</div>
		</div>
	{:else if currentSubView === 'colors'}
		<!-- Custom Colors Subview -->
		<div class="subview-container">
			<div class="subview-header">
				<button
					type="button"
					class="back-btn"
					onclick={() => (currentSubView = 'main')}
					aria-label={t('common.back') || 'Назад'}
				>
					<img
						src={chevronIcon}
						alt=""
						class="back-chevron"
						aria-hidden="true"
					/>
					<span class="subview-title">{t('settings.customColors')}</span>
				</button>
			</div>

			<div class="settings-group">
				<!-- Idle Color A (Dark) -->
				<SettingRow label={t('settings.shaderIdleColor')}>
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
						<span class="color-hex">
							{shaderSettings.idleColorA.toUpperCase()}
						</span>
					</div>
				</SettingRow>

				<!-- Idle Color B (Light) -->
				<SettingRow label={t('settings.shaderIdleColorB')}>
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
						<span class="color-hex">
							{shaderSettings.idleColorB.toUpperCase()}
						</span>
					</div>
				</SettingRow>

				<!-- Active Color A -->
				<SettingRow label={t('settings.shaderColorA')}>
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
									shaderSettings.setColorA(
										(e.target as HTMLInputElement).value,
									)}
							/>
						</label>
						<span class="color-hex">{shaderSettings.colorA.toUpperCase()}</span>
					</div>
				</SettingRow>

				<!-- Active Color B -->
				<SettingRow label={t('settings.shaderColorB')}>
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
									shaderSettings.setColorB(
										(e.target as HTMLInputElement).value,
									)}
							/>
						</label>
						<span class="color-hex">{shaderSettings.colorB.toUpperCase()}</span>
					</div>
				</SettingRow>

				<!-- Exposure Slider & Input -->
				<SettingRow label={t('settings.shaderExposure')}>
					<div class="range-control">
						<input
							type="range"
							min="0.8"
							max="2.5"
							step="0.05"
							aria-label={t('settings.shaderExposure')}
							value={shaderSettings.exposure}
							oninput={(e) => {
								const val = parseFloat((e.target as HTMLInputElement).value);
								shaderSettings.setExposure(val);
								if (!isEditingExposure) {
									exposureText = val.toFixed(2);
								}
							}}
							class="range-slider"
						/>
						<div class="range-input-wrapper">
							<input
								type="text"
								inputmode="decimal"
								class="range-text-input exposure-input"
								aria-label="{t('settings.shaderExposure')} (x)"
								bind:value={exposureText}
								onfocus={() => (isEditingExposure = true)}
								onblur={commitExposure}
								onkeydown={(e) => {
									if (e.key === 'Enter') {
										(e.target as HTMLInputElement).blur();
									}
								}}
							/>
							<span class="range-unit">x</span>
						</div>
					</div>
				</SettingRow>
			</div>

			<!-- Reset Button -->
			<div class="reset-row">
				<button
					type="button"
					class="reset-btn"
					onclick={() => shaderSettings.reset()}
				>
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
	{/if}
</div>

<style>
	.settings-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		gap: 14px;
		overflow-y: auto;
		padding-bottom: 96px;
		scrollbar-width: none;
		-ms-overflow-style: none;
	}
	.settings-view::-webkit-scrollbar {
		display: none;
	}

	.settings-section {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.section-header {
		padding: 0 4px;
	}

	.section-title {
		font-family: var(--font-family-base);
		font-size: 13px;
		font-weight: 500;
		color: #ffffff;
		letter-spacing: -0.1px;
	}

	.settings-group {
		display: flex;
		flex-direction: column;
		gap: 0;
		background: rgba(0, 0, 0, 0.2);
		border-radius: 16px;
		overflow: hidden;
	}

	.availability {
		font-family: var(--font-family-base);
		font-size: 11.5px;
		line-height: 1.35;
		color: rgba(255, 255, 255, 0.45);
		margin: 3px 4px 0 4px;
	}

	.badge {
		font-family: var(--font-family-base);
		font-size: 12px;
		color: #86e16f;
		background: rgba(134, 225, 111, 0.15);
		padding: 2px 8px;
		border-radius: 12px;
		font-weight: 500;
	}

	/* Subviews */
	.subview-container {
		display: flex;
		flex-direction: column;
		gap: 14px;
		width: 100%;
	}

	.subview-header {
		display: flex;
		align-items: center;
		margin-top: 2px;
		padding: 0 4px;
	}

	.back-btn {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		background: transparent;
		border: none;
		padding: 4px 0;
		cursor: pointer;
		color: #ffffff;
		transition: opacity 0.15s ease;
	}

	.back-btn:hover {
		opacity: 0.8;
	}

	.back-chevron {
		width: 14px;
		height: 14px;
		transform: rotate(90deg);
		opacity: 0.75;
		flex-shrink: 0;
		align-self: center;
		display: block;
	}

	.subview-title {
		font-family: var(--font-family-base);
		font-size: 15px;
		font-weight: 600;
		color: #ffffff;
	}

	.preset-item {
		width: 100%;
		min-height: 48px;
		background: transparent;
		border: none;
		border-bottom: 1px solid rgba(255, 255, 255, 0.06);
		padding: 10px 16px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		box-sizing: border-box;
		text-align: left;
		cursor: pointer;
		transition: background 0.15s ease;
	}

	.preset-item:first-child {
		border-top-left-radius: 16px;
		border-top-right-radius: 16px;
	}

	.preset-item:last-child {
		border-bottom-left-radius: 16px;
		border-bottom-right-radius: 16px;
		border-bottom: none;
	}

	.preset-item:hover {
		background: rgba(0, 0, 0, 0.4);
	}

	.preset-item:active {
		background: rgba(0, 0, 0, 0.5);
	}

	.preset-item.selected {
		background: rgba(0, 0, 0, 0.4);
	}

	.preset-item.selected:hover {
		background: rgba(0, 0, 0, 0.5);
	}

	.preset-left {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.swatches-pair {
		display: flex;
		align-items: center;
		position: relative;
		width: 28px;
		height: 18px;
	}

	.swatch-dot {
		width: 16px;
		height: 16px;
		border-radius: 50%;
		border: 1.5px solid rgba(0, 0, 0, 0.4);
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
		position: absolute;
		left: 0;
	}

	.swatch-overlap {
		left: 10px;
	}

	.preset-label {
		font-family: var(--font-family-base);
		font-size: 14px;
		font-weight: 500;
		color: #ffffff;
	}

	.check-mark {
		font-size: 15px;
		font-weight: 700;
		color: #ffffff;
	}

	/* Controls */
	.color-picker-control {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.color-swatch {
		width: 26px;
		height: 26px;
		border-radius: 8px;
		border: none;
		position: relative;
		cursor: pointer;
		overflow: hidden;
		box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
		transition: transform 0.15s ease;
		display: block;
	}

	.color-swatch:hover {
		transform: scale(1.1);
	}

	.color-swatch input[type='color'] {
		position: absolute;
		top: -10px;
		left: -10px;
		width: 50px;
		height: 50px;
		opacity: 0;
		cursor: pointer;
		padding: 0;
		margin: 0;
	}

	.color-hex {
		font-family: var(--font-family-base);
		font-size: 12px;
		color: rgba(255, 255, 255, 0.65);
		font-weight: 500;
		font-variant-numeric: tabular-nums;
		min-width: 60px;
	}

	.range-control {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
	}

	.range-slider {
		width: 90px;
		accent-color: #888899;
		cursor: pointer;
		height: 4px;
		background: rgba(255, 255, 255, 0.15);
		border-radius: 3px;
	}

	.range-input-wrapper {
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.2);
		border: 1px solid transparent;
		border-radius: 8px;
		padding: 2px 7px;
		height: 24px;
		box-sizing: border-box;
		transition:
			border-color 0.15s ease,
			background 0.15s ease;
	}

	.range-input-wrapper:hover,
	.range-input-wrapper:focus-within {
		border-color: rgba(255, 255, 255, 0.2);
		background: rgba(0, 0, 0, 0.35);
	}

	.range-text-input {
		width: 24px;
		background: transparent;
		border: none;
		padding: 0;
		margin: 0;
		color: #ffffff;
		font-family: var(--font-family-base);
		font-size: 12px;
		font-weight: 500;
		text-align: center;
		font-variant-numeric: tabular-nums;
		outline: none;
	}

	.range-text-input.exposure-input {
		width: 30px;
	}

	.range-unit {
		font-family: var(--font-family-base);
		font-size: 11px;
		color: rgba(255, 255, 255, 0.55);
		user-select: none;
		margin-left: 2px;
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
		background: rgba(0, 0, 0, 0.2);
		border: none;
		border-radius: 12px;
		padding: 8px 14px;
		color: rgba(255, 255, 255, 0.8);
		font-family: var(--font-family-base);
		font-size: 12px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s;
	}

	.reset-btn:hover {
		background: rgba(0, 0, 0, 0.4);
		color: #ffffff;
	}
</style>
