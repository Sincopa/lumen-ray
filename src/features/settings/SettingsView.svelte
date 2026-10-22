<script module lang="ts">
	/** Main-list position kept while the brightness screen replaces settings. */
	let resumeScroll: number | null = null;
</script>

<script lang="ts">
	import backIcon from '../../assets/chevron-left-small.svg';
	import { onMount, tick } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import SettingRow from '../../components/SettingRow.svelte';
	import ToggleSwitch from '../../components/ToggleSwitch.svelte';
	import { runtime } from '../../lib/runtime';
	import { supportsTun } from '../../lib/platform';
	import {
		nativeSettings,
		settingsError,
		updateNativeSettings,
		resetNetworkSettings,
	} from '../../lib/services/settings';
	import {
		i18n,
		t,
		SUPPORTED_LANGUAGES,
		type SupportedLanguage,
	} from '../../lib/i18n';
	import {
		shaderSettings,
		SHADER_PRESETS,
	} from '../../lib/stores/shaderSettings.svelte';
	import { applicationUpdate } from '../../lib/services/applicationUpdate';
	import { coreUpdate } from '../../lib/services/coreUpdate';
	import {
		getGeoDatabaseInfo,
		updateGeoDatabases,
		type GeoDatabaseInfo,
	} from '../../lib/services/geoDatabase';
	import { screen, reveal, pop } from '../../lib/motion';

	interface Props {
		onClose?: () => void;
		onOpenBrightness?: () => void;
	}
	let { onClose, onOpenBrightness }: Props = $props();

	type SubView =
		| 'main'
		| 'language'
		| 'uiScale'
		| 'appVersion'
		| 'notifications'
		| 'routingMode'
		| 'serverSelection'
		| 'dns'
		| 'updateInterval'
		| 'pingPolicy'
		| 'pingUrl'
		| 'resetProxy'
		| 'xrayCore'
		| 'geo'
		| 'logConsole'
		| 'presets'
		| 'colors';
	interface Option {
		value: string;
		label: string;
	}

	let resettingProxy = $state(false);
	let resetSuccess = $state(false);
	let currentSubView = $state<SubView>('main');
	const applicationUpdateState = applicationUpdate.state;
	const coreUpdateState = coreUpdate.state;
	let openingLogConsole = $state(false);
	let logConsoleError = $state(false);
	let customDnsOpen = $state(false);
	let customDnsDraft = $state('');
	let customDnsInvalid = $state(false);
	let geoInfo = $state<GeoDatabaseInfo | null>(null);
	let geoUpdating = $state(false);
	let geoUpdateSuccess = $state(false);
	let geoError = $state<string | null>(null);

	let scroller = $state<HTMLDivElement | null>(null);
	let mainScroll = 0;

	onMount(() => {
		void loadGeoInfo();
		if (resumeScroll !== null) {
			const top = resumeScroll;
			resumeScroll = null;
			void tick().then(() => {
				if (scroller) scroller.scrollTop = top;
			});
		}
	});

	function openBrightness() {
		resumeScroll = scroller?.scrollTop ?? 0;
		onOpenBrightness?.();
	}

	/** Opens a sub-page from the main list and remembers where the list was. */
	function open(view: SubView) {
		mainScroll = scroller?.scrollTop ?? 0;
		currentSubView = view;
		if (scroller) scroller.scrollTop = 0;
	}

	async function back() {
		if (currentSubView === 'main') {
			onClose?.();
			return;
		}
		currentSubView = 'main';
		await tick();
		if (scroller) scroller.scrollTop = mainScroll;
	}

	async function loadGeoInfo() {
		try {
			geoInfo = await getGeoDatabaseInfo();
			if (!geoInfo.valid && geoInfo.error) {
				geoError = geoInfo.error;
			}
		} catch {
			geoError = 'GEO_DATA_MISSING';
		}
	}

	async function handleUpdateGeo() {
		geoUpdating = true;
		geoError = null;
		geoUpdateSuccess = false;
		try {
			geoInfo = await updateGeoDatabases();
			if (!geoInfo.valid && geoInfo.error) {
				geoError = geoInfo.error;
			} else {
				geoUpdateSuccess = true;
				setTimeout(() => {
					geoUpdateSuccess = false;
				}, 4000);
			}
		} catch (e) {
			geoError = e instanceof Error ? e.message : 'GEO_DATA_MISSING';
		} finally {
			geoUpdating = false;
		}
	}

	let exposureText = $state(shaderSettings.exposure.toFixed(2));
	let isEditingExposure = $state(false);

	$effect(() => {
		if (!isEditingExposure) {
			exposureText = shaderSettings.exposure.toFixed(2);
		}
	});

	function handleUpdateAction() {
		if ($applicationUpdateState.status === 'available') {
			void applicationUpdate.install();
		} else {
			void applicationUpdate.check();
		}
	}

	function handleCoreUpdateAction() {
		if ($coreUpdateState.status === 'available') void coreUpdate.install();
		else void coreUpdate.check();
	}

	async function openLogConsole() {
		if (!runtime.isTauri || openingLogConsole) return;
		openingLogConsole = true;
		logConsoleError = false;
		try {
			await invoke('open_log_console');
		} catch {
			logConsoleError = true;
		} finally {
			openingLogConsole = false;
		}
	}

	function commitCustomDns() {
		const value = customDnsDraft.trim();
		const octets = value.split('.');
		const valid =
			octets.length === 4 &&
			octets.every(
				(octet) => /^\d{1,3}$/.test(octet) && Number(octet) <= 255,
			) &&
			value !== '0.0.0.0' &&
			value !== '255.255.255.255' &&
			Number(octets[0]) < 224;
		customDnsInvalid = !valid;
		if (valid) void updateNativeSettings({ dnsServer: value });
	}

	async function handleResetProxy() {
		if (!runtime.isTauri || resettingProxy) return;
		resettingProxy = true;
		resetSuccess = false;
		try {
			await resetNetworkSettings();
			resetSuccess = true;
			setTimeout(() => {
				resetSuccess = false;
			}, 3000);
		} catch (e) {
			console.error('Failed to reset proxy:', e);
		} finally {
			resettingProxy = false;
		}
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

	const uiScaleOptions = [75, 90, 100, 110, 125, 150].map((percent) => ({
		value: String(percent),
		label: `${percent}%`,
	}));
	let uiScaleValue = $derived(String(Math.round(shaderSettings.uiScale * 100)));

	let routingOptions = $derived([
		...(supportsTun ? [{ value: 'tun', label: t('settings.modeTun') }] : []),
		{ value: 'proxy', label: t('settings.modeProxy') },
	]);
	let networkMode = $derived(
		runtime.isTauri && supportsTun ? $nativeSettings.networkMode : 'proxy',
	);

	let dnsOptions = $derived([
		{ value: 'system', label: t('settings.dnsSystem') },
		{ value: '1.1.1.1', label: t('settings.dnsCloudflare') },
		{ value: '8.8.8.8', label: t('settings.dnsGoogle') },
		{ value: '94.140.14.14', label: t('settings.dnsAdGuard') },
		{ value: '9.9.9.9', label: t('settings.dnsQuad9') },
		{ value: 'custom', label: t('settings.dnsCustom') },
	]);
	let dnsSelection = $derived(
		['system', '1.1.1.1', '8.8.8.8', '94.140.14.14', '9.9.9.9'].includes(
			$nativeSettings.dnsServer,
		)
			? $nativeSettings.dnsServer
			: 'custom',
	);
	$effect(() => {
		if (dnsSelection === 'custom' && !customDnsOpen) {
			customDnsDraft = $nativeSettings.dnsServer;
		}
	});

	let serverSelectionOptions = $derived([
		{ value: 'manual', label: t('settings.serverManual') },
		{ value: 'best', label: t('settings.serverBest') },
	]);
	let pingPolicyOptions = $derived([
		{ value: 'startup', label: t('settings.pingStartup') },
		{ value: 'list', label: t('settings.pingList') },
		{ value: 'manual', label: t('settings.pingManual') },
	]);

	let updateIntervalOptions = $derived([
		{ value: '1', label: t('settings.update1h') },
		{ value: '6', label: t('settings.update6h') },
		{ value: '24', label: t('settings.update24h') },
		{ value: '0', label: t('settings.updateManual') },
	]);
	let updateInterval = $derived(
		runtime.isTauri
			? ($nativeSettings.updateIntervalHours?.toString() ?? '24')
			: '24',
	);
	const updateIntervalShort: Record<string, string> = {
		'1': 'settings.update1hShort',
		'6': 'settings.update6hShort',
		'24': 'settings.update24hShort',
		'0': 'settings.updateManualShort',
	};

	function appUpdateLabel() {
		switch ($applicationUpdateState.status) {
			case 'checking':
				return t('settings.checkingUpdates');
			case 'downloading':
				return t('settings.downloadingUpdate');
			case 'available':
				return t('settings.downloadUpdate', {
					version: $applicationUpdateState.version ?? '',
				});
			case 'latest':
				return t('settings.latestVersionInstalled');
			case 'error':
				return t('common.retry');
			default:
				return t('settings.checkUpdates');
		}
	}

	function coreUpdateLabel() {
		switch ($coreUpdateState.status) {
			case 'checking':
				return t('settings.checkingUpdates');
			case 'installing':
				return t('settings.downloadingUpdate');
			case 'available':
				return t('settings.downloadUpdate', {
					version: $coreUpdateState.version ?? '',
				});
			case 'latest':
				return t('settings.latestVersionInstalled');
			case 'error':
				return t('common.retry');
			default:
				return t('settings.checkUpdates');
		}
	}

	let titles = $derived<Record<SubView, string>>({
		main: '',
		language: t('settings.language'),
		uiScale: t('settings.uiScale'),
		appVersion: t('settings.appVersion'),
		notifications: t('settings.notifications'),
		routingMode: t('settings.routingMode'),
		serverSelection: t('settings.serverSelection'),
		dns: t('settings.dnsServer'),
		updateInterval: t('settings.updateInterval'),
		pingPolicy: t('settings.pingPolicy'),
		pingUrl: t('settings.pingUrl'),
		resetProxy: t('settings.resetProxy'),
		xrayCore: t('settings.xrayCore'),
		geo: t('settings.geoDatabases'),
		logConsole: t('settings.logConsole'),
		presets: t('settings.presets'),
		colors: t('settings.customColors'),
	});
</script>

{#snippet choices(
	options: Option[],
	value: string,
	onSelect: (value: string) => void,
	disabled = false,
)}
	<div class="settings-group choice-group" role="radiogroup">
		{#each options as option (option.value)}
			<button
				type="button"
				role="radio"
				class="choice-row"
				aria-checked={value === option.value}
				{disabled}
				onclick={() => value !== option.value && onSelect(option.value)}
			>
				<span class="choice-mark" aria-hidden="true">
					{#if value === option.value}
						<span
							class="choice-dot"
							in:pop={{ start: 0.4, duration: 220 }}
						></span>
					{/if}
				</span>
				<span class="choice-label">{option.label}</span>
			</button>
		{/each}
	</div>
{/snippet}

{#snippet actionCard(
	label: string,
	value: string,
	action: string,
	onclick: () => void,
	disabled: boolean,
	success = false,
	error = false,
)}
	<div class="settings-group action-card">
		<div class="info-row">
			<span class="info-label">{label}</span>
			<span class="info-value" class:error-text={error}>{value}</span>
		</div>
		<button type="button" class="wide-btn" class:success {disabled} {onclick}>
			{action}
		</button>
	</div>
{/snippet}

{#snippet hint(text: string | undefined, alert = false)}
	{#if text}
		<p class="hint" role={alert ? 'alert' : undefined}>{text}</p>
	{/if}
{/snippet}

<div class="settings-view" bind:this={scroller}>
	{#key currentSubView}
		<div
			class="settings-page"
			in:screen={{ dir: currentSubView === 'main' ? -1 : 1 }}
		>
			<div class="page-header">
				<button
					type="button"
					class="circle-back"
					aria-label={t('common.back')}
					onclick={() => void back()}
				>
					<img src={backIcon} alt="" width="24" height="24" />
				</button>
				{#if currentSubView !== 'main'}
					<span class="page-title">{titles[currentSubView]}</span>
				{/if}
			</div>

			{#if currentSubView === 'main'}
				<section class="settings-section">
					<h2 class="section-title">{t('settings.generalSection')}</h2>
					<div class="settings-group">
						<SettingRow
							label={t('settings.language')}
							showChevron
							onclick={() => open('language')}
						/>
						<SettingRow label={t('settings.animation')}>
							<ToggleSwitch
								mode="checkbox"
								ariaLabel={t('settings.animation')}
								checked={shaderSettings.enabled}
								onchange={(val) => shaderSettings.setEnabled(val)}
							/>
						</SettingRow>
						<SettingRow label={t('settings.tooltips')}>
							<ToggleSwitch
								mode="checkbox"
								ariaLabel={t('settings.tooltips')}
								checked={shaderSettings.tooltipsEnabled}
								onchange={(val) => shaderSettings.setTooltipsEnabled(val)}
							/>
						</SettingRow>
						<SettingRow
							label={t('settings.uiScale')}
							value="{uiScaleValue}%"
							showChevron
							onclick={() => open('uiScale')}
						/>
						<SettingRow
							label={t('settings.appVersion')}
							showChevron
							onclick={() => open('appVersion')}
						/>
					</div>
				</section>

				<section class="settings-section">
					<h2 class="section-title">{t('settings.systemSection')}</h2>
					<div class="settings-group">
						<SettingRow label={t('settings.autoStart')}>
							<ToggleSwitch
								mode="checkbox"
								disabled={!runtime.isTauri}
								ariaLabel={t('settings.autoStart')}
								checked={runtime.isTauri ? $nativeSettings.autoStart : false}
								onchange={(val) => updateNativeSettings({ autoStart: val })}
							/>
						</SettingRow>
						<SettingRow label={t('settings.closeToTray')}>
							<ToggleSwitch
								mode="checkbox"
								disabled={!runtime.isTauri}
								ariaLabel={t('settings.closeToTray')}
								checked={runtime.isTauri ? $nativeSettings.closeToTray : true}
								onchange={(val) => updateNativeSettings({ closeToTray: val })}
							/>
						</SettingRow>
						<SettingRow label={t('settings.startMinimized')}>
							<ToggleSwitch
								mode="checkbox"
								disabled={!runtime.isTauri}
								ariaLabel={t('settings.startMinimized')}
								checked={runtime.isTauri
									? $nativeSettings.startMinimized
									: false}
								onchange={(val) =>
									updateNativeSettings({ startMinimized: val })}
							/>
						</SettingRow>
						<SettingRow
							label={t('settings.notifications')}
							showChevron
							onclick={() => open('notifications')}
						/>
					</div>
				</section>

				<section class="settings-section">
					<h2 class="section-title">{t('settings.networkSection')}</h2>
					<div class="settings-group">
						<SettingRow
							label={t('settings.routingMode')}
							value={t(
								networkMode === 'tun'
									? 'settings.modeTunShort'
									: 'settings.modeProxyShort',
							)}
							showChevron
							onclick={() => open('routingMode')}
						/>
						<SettingRow label={t('settings.autoConnect')}>
							<ToggleSwitch
								mode="checkbox"
								disabled={!runtime.isTauri}
								ariaLabel={t('settings.autoConnect')}
								checked={runtime.isTauri ? $nativeSettings.autoConnect : false}
								onchange={(value) =>
									updateNativeSettings({ autoConnect: value })}
							/>
						</SettingRow>
						<SettingRow label={t('settings.autoReconnect')}>
							<ToggleSwitch
								mode="checkbox"
								disabled={!runtime.isTauri}
								ariaLabel={t('settings.autoReconnect')}
								checked={$nativeSettings.autoReconnect}
								onchange={(value) =>
									updateNativeSettings({ autoReconnect: value })}
							/>
						</SettingRow>
						<SettingRow
							label={t('settings.serverSelection')}
							value={t(
								$nativeSettings.serverSelection === 'best'
									? 'settings.serverBestShort'
									: 'settings.serverManual',
							)}
							showChevron
							onclick={() => open('serverSelection')}
						/>
						<SettingRow
							label={t('settings.dnsServer')}
							showChevron
							onclick={() => open('dns')}
						/>
						<SettingRow label={t('settings.bypassLan')}>
							<ToggleSwitch
								mode="checkbox"
								disabled={!runtime.isTauri}
								ariaLabel={t('settings.bypassLan')}
								checked={runtime.isTauri
									? ($nativeSettings.bypassLan ?? true)
									: true}
								onchange={(val) => updateNativeSettings({ bypassLan: val })}
							/>
						</SettingRow>
						<SettingRow
							label={t('settings.updateInterval')}
							value={t(updateIntervalShort[updateInterval] ?? '')}
							showChevron
							onclick={() => open('updateInterval')}
						/>
						<SettingRow
							label={t('settings.pingPolicy')}
							showChevron
							onclick={() => open('pingPolicy')}
						/>
						<SettingRow
							label={t('settings.pingUrl')}
							showChevron
							onclick={() => open('pingUrl')}
						/>
						<SettingRow
							label={t('settings.resetProxy')}
							showChevron
							onclick={() => open('resetProxy')}
						/>
						<SettingRow
							label={t('settings.xrayCore')}
							showChevron
							onclick={() => open('xrayCore')}
						/>
						<SettingRow
							label={t('settings.geoDatabases')}
							showChevron
							onclick={() => open('geo')}
						/>
						<SettingRow
							label={t('settings.logConsole')}
							showChevron
							onclick={() => open('logConsole')}
						/>
					</div>
					{@render hint(
						runtime.isTauri
							? $settingsError
								? t('errors.' + $settingsError)
								: ''
							: t('demo.unavailable'),
					)}
				</section>

				<section class="settings-section">
					<h2 class="section-title">{t('settings.shaderTitle')}</h2>
					<div class="settings-group">
						{#if onOpenBrightness}
							<SettingRow
								label={t('settings.brightness')}
								showChevron
								onclick={openBrightness}
							/>
						{/if}
						<SettingRow
							label={t('settings.presets')}
							showChevron
							onclick={() => open('presets')}
						/>
						<SettingRow
							label={t('settings.customColors')}
							showChevron
							onclick={() => open('colors')}
						/>
					</div>
				</section>
			{:else if currentSubView === 'language'}
				{@render choices(languageOptions, i18n.currentLanguage, (value) =>
					i18n.setLanguage(value as SupportedLanguage),
				)}
			{:else if currentSubView === 'uiScale'}
				{@render choices(uiScaleOptions, uiScaleValue, (value) =>
					shaderSettings.setUiScale(Number(value) / 100),
				)}
				{@render hint(t('settings.uiScaleDesc'))}
			{:else if currentSubView === 'appVersion'}
				{@render actionCard(
					t('settings.currentVersion'),
					$applicationUpdateState.currentVersion,
					appUpdateLabel(),
					handleUpdateAction,
					['checking', 'downloading'].includes($applicationUpdateState.status),
					$applicationUpdateState.status === 'latest',
				)}
			{:else if currentSubView === 'notifications'}
				<div class="settings-group">
					<SettingRow label={t('settings.notifyDisconnect')}>
						<ToggleSwitch
							mode="checkbox"
							disabled={!runtime.isTauri}
							ariaLabel={t('settings.notifyDisconnect')}
							checked={$nativeSettings.notifyDisconnect}
							onchange={(value) =>
								updateNativeSettings({ notifyDisconnect: value })}
						/>
					</SettingRow>
					<SettingRow label={t('settings.notifyReconnect')}>
						<ToggleSwitch
							mode="checkbox"
							disabled={!runtime.isTauri}
							ariaLabel={t('settings.notifyReconnect')}
							checked={$nativeSettings.notifyReconnect}
							onchange={(value) =>
								updateNativeSettings({ notifyReconnect: value })}
						/>
					</SettingRow>
					<SettingRow label={t('settings.notifyExpiry')}>
						<ToggleSwitch
							mode="checkbox"
							disabled={!runtime.isTauri}
							ariaLabel={t('settings.notifyExpiry')}
							checked={$nativeSettings.notifyExpiry}
							onchange={(value) =>
								updateNativeSettings({ notifyExpiry: value })}
						/>
					</SettingRow>
					<SettingRow label={t('settings.notifyUpdate')}>
						<ToggleSwitch
							mode="checkbox"
							disabled={!runtime.isTauri}
							ariaLabel={t('settings.notifyUpdate')}
							checked={$nativeSettings.notifyUpdate}
							onchange={(value) =>
								updateNativeSettings({ notifyUpdate: value })}
						/>
					</SettingRow>
				</div>
			{:else if currentSubView === 'routingMode'}
				{@render choices(
					routingOptions,
					networkMode,
					(value) =>
						updateNativeSettings({ networkMode: value as 'proxy' | 'tun' }),
					!runtime.isTauri,
				)}
				{@render hint(
					t(supportsTun ? 'settings.routingDesc' : 'settings.routingProxyOnly'),
				)}
			{:else if currentSubView === 'serverSelection'}
				{@render choices(
					serverSelectionOptions,
					$nativeSettings.serverSelection,
					(value) =>
						updateNativeSettings({
							serverSelection: value as 'manual' | 'best',
						}),
					!runtime.isTauri,
				)}
				{@render hint(t('settings.serverSelectionDesc'))}
			{:else if currentSubView === 'dns'}
				{@render choices(
					dnsOptions,
					runtime.isTauri
						? customDnsOpen
							? 'custom'
							: dnsSelection
						: 'system',
					(value) => {
						customDnsOpen = value === 'custom';
						customDnsInvalid = false;
						if (value !== 'custom')
							void updateNativeSettings({ dnsServer: value });
					},
					!runtime.isTauri,
				)}
				{#if customDnsOpen || dnsSelection === 'custom'}
					<div class="settings-group field-group" transition:reveal>
						<input
							class="text-field"
							type="text"
							inputmode="decimal"
							placeholder="9.9.9.9"
							aria-label={t('settings.dnsCustomAddress')}
							bind:value={customDnsDraft}
							onchange={commitCustomDns}
							onkeydown={(event) => {
								if (event.key === 'Enter')
									(event.target as HTMLInputElement).blur();
							}}
						/>
					</div>
					{@render hint(
						customDnsInvalid
							? t('settings.dnsInvalid')
							: t('settings.dnsCustomHint'),
						customDnsInvalid,
					)}
				{/if}
				{@render hint(supportsTun ? t('settings.dnsTunOnly') : undefined)}
			{:else if currentSubView === 'updateInterval'}
				{@render choices(
					updateIntervalOptions,
					updateInterval,
					(value) =>
						updateNativeSettings({ updateIntervalHours: parseInt(value, 10) }),
					!runtime.isTauri,
				)}
			{:else if currentSubView === 'pingPolicy'}
				{@render choices(
					pingPolicyOptions,
					$nativeSettings.pingPolicy,
					(value) =>
						updateNativeSettings({
							pingPolicy: value as 'startup' | 'list' | 'manual',
						}),
					!runtime.isTauri,
				)}
			{:else if currentSubView === 'pingUrl'}
				<div class="settings-group field-group">
					<input
						type="text"
						class="text-field"
						placeholder="http://www.google.com/generate_204"
						aria-label={t('settings.pingUrl')}
						spellcheck="false"
						value={$nativeSettings.pingUrl || ''}
						onchange={(e) =>
							updateNativeSettings({
								pingUrl: (e.target as HTMLInputElement).value.trim(),
							})}
						onkeydown={(event) => {
							if (event.key === 'Enter')
								(event.target as HTMLInputElement).blur();
						}}
					/>
				</div>
				{@render hint(t('settings.pingUrlDesc'))}
			{:else if currentSubView === 'resetProxy'}
				<div class="settings-group action-card">
					<p class="card-text">{t('settings.resetProxyDesc')}</p>
					<button
						type="button"
						class="wide-btn"
						class:success={resetSuccess}
						disabled={!runtime.isTauri || resettingProxy}
						onclick={handleResetProxy}
					>
						{resetSuccess
							? t('settings.proxyResetSuccess')
							: t('settings.resetProxyAction')}
					</button>
				</div>
			{:else if currentSubView === 'xrayCore'}
				{@render actionCard(
					t('settings.currentVersion'),
					runtime.isTauri
						? $nativeSettings.coreVersion
						: t('settings.mockReady'),
					coreUpdateLabel(),
					handleCoreUpdateAction,
					!runtime.isTauri ||
						['checking', 'installing'].includes($coreUpdateState.status),
					$coreUpdateState.status === 'latest',
				)}
			{:else if currentSubView === 'geo'}
				{@render actionCard(
					t('settings.currentVersion'),
					geoInfo?.version || '—',
					geoUpdating
						? t('settings.downloadingUpdate')
						: geoUpdateSuccess
							? t('settings.latestVersionInstalled')
							: t('settings.updateDatabases'),
					handleUpdateGeo,
					geoUpdating,
					geoUpdateSuccess,
					Boolean(geoError || (geoInfo && !geoInfo.valid)),
				)}
				{@render hint(
					geoError
						? t('errors.' + geoError) || t('settings.geoDataCorrupted')
						: geoInfo
							? `${geoInfo.source}${geoInfo.updatedAt ? ' · ' + new Date(geoInfo.updatedAt).toLocaleDateString() : ''}`
							: undefined,
					Boolean(geoError),
				)}
			{:else if currentSubView === 'logConsole'}
				<div class="settings-group action-card">
					<p class="card-text">{t('settings.logConsoleDesc')}</p>
					<button
						type="button"
						class="wide-btn"
						disabled={!runtime.isTauri || openingLogConsole}
						onclick={() => void openLogConsole()}
					>
						{openingLogConsole
							? t('common.loading')
							: t('settings.openLogConsole')}
					</button>
				</div>
				{#if logConsoleError}
					{@render hint(t('settings.logConsoleError'), true)}
				{/if}
			{:else if currentSubView === 'presets'}
				<div class="settings-group choice-group" role="radiogroup">
					{#each SHADER_PRESETS as preset (preset.id)}
						{@const selected = shaderSettings.selectedPreset === preset.id}
						<button
							type="button"
							role="radio"
							class="choice-row"
							aria-checked={selected}
							onclick={() => shaderSettings.applyPreset(preset.id)}
						>
							<span class="choice-mark" aria-hidden="true">
								{#if selected}
									<span
										class="choice-dot"
										in:pop={{ start: 0.4, duration: 220 }}
									></span>
								{/if}
							</span>
							<span class="choice-label">{t(preset.nameKey)}</span>
							<span class="swatches-pair" aria-hidden="true">
								<span
									class="swatch-dot"
									style:background={preset.colorA}
								></span>
								<span
									class="swatch-dot swatch-overlap"
									style:background={preset.colorB}
								></span>
							</span>
						</button>
					{/each}
					{#if shaderSettings.selectedPreset === 'custom'}
						<div class="choice-row" role="radio" aria-checked="true">
							<span class="choice-mark" aria-hidden="true">
								<span class="choice-dot"></span>
							</span>
							<span class="choice-label">
								{t('settings.shaderPresetCustom')}
							</span>
							<span class="swatches-pair" aria-hidden="true">
								<span
									class="swatch-dot"
									style:background={shaderSettings.colorA}
								></span>
								<span
									class="swatch-dot swatch-overlap"
									style:background={shaderSettings.colorB}
								></span>
							</span>
						</div>
					{/if}
				</div>
			{:else if currentSubView === 'colors'}
				{#snippet colorRow(
					label: string,
					value: string,
					onInput: (hex: string) => void,
				)}
					<SettingRow {label}>
						<div class="color-picker-control">
							<span class="color-hex">{value.toUpperCase()}</span>
							<label class="color-swatch" style:background-color={value}>
								<input
									type="color"
									aria-label={label}
									{value}
									oninput={(e) => onInput((e.target as HTMLInputElement).value)}
								/>
							</label>
						</div>
					</SettingRow>
				{/snippet}
				<div class="settings-group">
					{@render colorRow(
						t('settings.shaderIdleColor'),
						shaderSettings.idleColorA,
						(hex) => shaderSettings.setIdleColorA(hex),
					)}
					{@render colorRow(
						t('settings.shaderIdleColorB'),
						shaderSettings.idleColorB,
						(hex) => shaderSettings.setIdleColorB(hex),
					)}
					{@render colorRow(
						t('settings.shaderColorA'),
						shaderSettings.colorA,
						(hex) => shaderSettings.setColorA(hex),
					)}
					{@render colorRow(
						t('settings.shaderColorB'),
						shaderSettings.colorB,
						(hex) => shaderSettings.setColorB(hex),
					)}
					<SettingRow label={t('settings.shaderExposure')}>
						<input
							type="text"
							inputmode="decimal"
							class="value-input"
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
					</SettingRow>
				</div>
				<button
					type="button"
					class="wide-btn"
					onclick={() => shaderSettings.reset()}
				>
					{t('settings.shaderReset')}
				</button>
			{/if}
		</div>
	{/key}
</div>

<style>
	.settings-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		overflow-y: auto;
		padding-bottom: 32px;
		scrollbar-width: none;
		-ms-overflow-style: none;
	}
	.settings-view::-webkit-scrollbar {
		display: none;
	}
	.settings-page {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
	}

	/* Back button with the page title centred on the window. */
	.page-header {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 40px;
		margin-top: 5px;
	}
	.circle-back {
		position: absolute;
		left: 0;
		top: 0;
		display: grid;
		place-items: center;
		width: 40px;
		height: 40px;
		padding: 0;
		border: 1px solid rgba(0, 0, 0, 0.15);
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.05);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		cursor: pointer;
		transition:
			background-color 0.15s ease,
			transform 160ms var(--ease-spring, ease);
	}
	.circle-back:active {
		transform: scale(0.94);
	}
	.page-title,
	.section-title {
		margin: 0;
		font-family: var(--font-family-base);
		font-size: 14px;
		font-weight: 500;
		letter-spacing: -0.02em;
		line-height: 1;
		color: rgba(255, 255, 255, 0.5);
		text-align: center;
		text-box-trim: trim-both;
		text-box-edge: cap alphabetic;
	}
	/* Ellipsis needs overflow clipping, so this title keeps its full line box. */
	.page-title {
		max-width: calc(100% - 112px);
		line-height: 1.3;
		text-box-trim: none;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.settings-section {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.settings-section + .settings-section {
		margin-top: 12px;
	}

	.settings-group {
		display: flex;
		flex-direction: column;
		padding: 0 18px;
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid rgba(0, 0, 0, 0.25);
		border-radius: 24px;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}
	.settings-group > :global(* + *) {
		border-top: 1px solid rgba(255, 255, 255, 0.05);
	}

	.hint {
		margin: 0 8px;
		font-family: var(--font-family-base);
		font-size: 12px;
		line-height: 1.4;
		color: rgba(255, 255, 255, 0.45);
	}
	.hint[role='alert'] {
		color: #feb2b2;
	}

	/* Single-choice lists: a white dot marks the current option. */
	.choice-group {
		padding: 0 14px;
	}
	.choice-group > :global(* + *) {
		border-top: none;
	}
	.choice-row {
		position: relative;
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		min-height: 54px;
		padding: 0;
		border: 0;
		background: transparent;
		color: #fff;
		text-align: left;
		cursor: pointer;
	}
	.choice-row + .choice-row::before {
		content: '';
		position: absolute;
		top: 0;
		left: 40px;
		right: 0;
		border-top: 1px solid rgba(255, 255, 255, 0.05);
	}
	.choice-row:disabled {
		cursor: not-allowed;
		opacity: 0.6;
	}
	.choice-mark {
		display: grid;
		place-items: center;
		flex: 0 0 32px;
		height: 32px;
	}
	.choice-dot {
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: #fff;
	}
	.choice-label {
		flex: 1;
		min-width: 0;
		font-family: var(--font-family-base);
		font-size: 14px;
		letter-spacing: -0.02em;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	@media (hover: hover) and (pointer: fine) {
		.choice-row:not(:disabled):hover
			.choice-mark:not(:has(.choice-dot))::after {
			content: '';
			width: 16px;
			height: 16px;
			border-radius: 50%;
			background: rgba(255, 255, 255, 0.15);
		}
	}

	/* Version card: value row and a wide action button. */
	.action-card {
		gap: 16px;
		padding: 18px;
	}
	.action-card > :global(* + *) {
		border-top: none;
	}
	.info-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 6px 8px;
	}
	.info-label {
		font-family: var(--font-family-base);
		font-size: 12px;
		color: rgba(255, 255, 255, 0.5);
	}
	.info-value {
		font-family: var(--font-family-base);
		font-size: 14px;
		font-weight: 500;
		font-variant-numeric: tabular-nums;
		color: #fff;
		text-align: right;
		overflow-wrap: anywhere;
	}
	.info-value.error-text {
		color: #feb2b2;
	}
	.card-text {
		margin: 0;
		padding: 0 8px;
		font-family: var(--font-family-base);
		font-size: 13px;
		line-height: 1.45;
		color: rgba(255, 255, 255, 0.6);
	}
	.wide-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		min-height: 48px;
		padding: 12px 24px;
		border: 1px solid rgba(255, 255, 255, 0.05);
		border-radius: 123px;
		background: rgba(255, 255, 255, 0.05);
		color: rgba(255, 255, 255, 0.8);
		font-family: var(--font-family-base);
		font-size: 16px;
		font-weight: 500;
		cursor: pointer;
		transition:
			background-color 0.15s ease,
			color 0.15s ease,
			transform 160ms var(--ease-spring, ease);
	}
	.wide-btn:active:not(:disabled) {
		transform: scale(0.98);
	}
	.wide-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.wide-btn.success {
		color: #86e16f;
		background: rgba(134, 225, 111, 0.1);
	}

	.field-group {
		padding: 0 18px;
	}
	.text-field,
	.value-input {
		width: 100%;
		min-height: 54px;
		padding: 0;
		border: 0;
		background: transparent;
		color: #fff;
		font-family: var(--font-family-base);
		font-size: 14px;
		letter-spacing: -0.02em;
		outline: none;
	}
	.text-field::placeholder {
		color: rgba(255, 255, 255, 0.35);
	}
	.value-input {
		width: 64px;
		min-height: 33px;
		padding: 0 12px;
		border-radius: 123px;
		background: rgba(0, 0, 0, 0.15);
		font-size: 12px;
		text-align: center;
		font-variant-numeric: tabular-nums;
	}

	.swatches-pair {
		position: relative;
		flex: 0 0 30px;
		height: 18px;
	}
	.swatch-dot {
		position: absolute;
		top: 1px;
		left: 0;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		box-shadow: 0 0 0 1.5px rgba(0, 0, 0, 0.4);
	}
	.swatch-overlap {
		left: 12px;
	}

	.color-picker-control {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.color-swatch {
		position: relative;
		display: block;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		overflow: hidden;
		cursor: pointer;
		box-shadow: 0 2px 7.7px rgba(0, 0, 0, 0.25);
	}
	.color-swatch input[type='color'] {
		position: absolute;
		inset: -10px;
		width: 50px;
		height: 50px;
		padding: 0;
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}
	.color-hex {
		font-family: var(--font-family-base);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		color: rgba(255, 255, 255, 0.6);
	}

	@media (hover: hover) and (pointer: fine) {
		.circle-back:hover {
			background: rgba(255, 255, 255, 0.1);
		}
		.wide-btn:hover:not(:disabled) {
			background: rgba(255, 255, 255, 0.09);
			color: #fff;
		}
	}
</style>
