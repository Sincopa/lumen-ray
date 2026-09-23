<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import chevronIcon from '../../../assets/chevron.svg';
	import reloadIcon from '../../../assets/reload.svg';
	import Dropdown from '../../components/Dropdown.svelte';
	import SettingRow from '../../components/SettingRow.svelte';
	import ToggleSwitch from '../../components/ToggleSwitch.svelte';
	import { runtime } from '../../runtime';
	import {
		nativeSettings,
		settingsError,
		updateNativeSettings,
		resetNetworkSettings,
	} from '../../services/settings';
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
	import { applicationUpdate } from '../../services/applicationUpdate';
	import { coreUpdate } from '../../services/coreUpdate';
	import {
		getGeoDatabaseInfo,
		updateGeoDatabases,
		type GeoDatabaseInfo,
	} from '../../services/geoDatabase';

	interface NetworkDiagnostics {
		status: string;
		networkMode: string;
		coreVersion: string;
		publicIpv4: string | null;
		publicIpv6: string | null;
		systemDns: string[];
		configuredDns: string;
		tunnelReady: boolean;
		routesReady: boolean;
		routingConflict: boolean;
		systemProxyEnabled: boolean;
		proxyManaged: boolean;
	}

	let autoConnect = $state(false);
	let resettingProxy = $state(false);
	let resetSuccess = $state(false);
	let currentSubView = $state<
		'main' | 'presets' | 'colors' | 'notifications' | 'diagnostics'
	>('main');
	const applicationUpdateState = applicationUpdate.state;
	const coreUpdateState = coreUpdate.state;
	let diagnostics = $state<NetworkDiagnostics | null>(null);
	let diagnosticsBusy = $state(false);
	let diagnosticsError = $state(false);
	let reportCopied = $state(false);
	let openingLogConsole = $state(false);
	let logConsoleError = $state(false);
	let customDnsOpen = $state(false);
	let customDnsDraft = $state('');
	let customDnsInvalid = $state(false);
	let geoInfo = $state<GeoDatabaseInfo | null>(null);
	let geoUpdating = $state(false);
	let geoUpdateSuccess = $state(false);
	let geoError = $state<string | null>(null);

	onMount(() => {
		void loadGeoInfo();
	});

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

	let uiScaleText = $state(Math.round(shaderSettings.uiScale * 100).toString());
	let uiScaleDraft = $state(shaderSettings.uiScale);
	let exposureText = $state(shaderSettings.exposure.toFixed(2));
	let isEditingUiScale = $state(false);
	let isEditingExposure = $state(false);

	$effect(() => {
		if (!isEditingUiScale) {
			uiScaleText = Math.round(shaderSettings.uiScale * 100).toString();
			uiScaleDraft = shaderSettings.uiScale;
		}
	});

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

	async function loadDiagnostics() {
		if (!runtime.isTauri) return;
		diagnosticsBusy = true;
		diagnosticsError = false;
		try {
			diagnostics = await invoke<NetworkDiagnostics>('get_network_diagnostics');
		} catch {
			diagnosticsError = true;
		} finally {
			diagnosticsBusy = false;
		}
	}

	async function copyDiagnostics() {
		if (!diagnostics) return;
		const report = [
			`Lumen / Xray ${diagnostics.coreVersion}`,
			`Status: ${diagnostics.status}`,
			`Mode: ${diagnostics.networkMode}`,
			`IPv4: ${diagnostics.publicIpv4 ?? 'unavailable'}`,
			`IPv6: ${diagnostics.publicIpv6 ?? 'unavailable'}`,
			`System DNS: ${diagnostics.systemDns.join(', ') || 'unavailable'}`,
			`Selected DNS: ${diagnostics.configuredDns}`,
			`TUN adapter: ${diagnostics.tunnelReady}`,
			`TUN routes: ${diagnostics.routesReady}`,
			`Routing conflict: ${diagnostics.routingConflict}`,
			`System proxy enabled: ${diagnostics.systemProxyEnabled}`,
			`System proxy managed: ${diagnostics.proxyManaged}`,
		].join('\n');
		try {
			await navigator.clipboard.writeText(report);
			reportCopied = true;
			setTimeout(() => {
				reportCopied = false;
			}, 2000);
		} catch {
			diagnosticsError = true;
		}
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

	function commitUiScale() {
		isEditingUiScale = false;
		const parsed = parseInt(uiScaleText, 10);
		if (isNaN(parsed)) {
			uiScaleText = Math.round(shaderSettings.uiScale * 100).toString();
			return;
		}
		const clamped = Math.min(150, Math.max(75, parsed));
		shaderSettings.setUiScale(clamped / 100);
		uiScaleDraft = clamped / 100;
		uiScaleText = clamped.toString();
	}

	function commitUiScaleSlider() {
		shaderSettings.setUiScale(uiScaleDraft);
		uiScaleText = Math.round(uiScaleDraft * 100).toString();
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
							value={uiScaleDraft}
							oninput={(e) => {
								const val = parseFloat((e.target as HTMLInputElement).value);
								uiScaleDraft = val;
								if (!isEditingUiScale) {
									uiScaleText = Math.round(val * 100).toString();
								}
							}}
							onchange={commitUiScaleSlider}
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

				<!-- App Version & Updates -->
				<SettingRow label={t('settings.appVersion')}>
					<div class="version-badge-container">
						<span class="badge version-badge">
							v{$applicationUpdateState.currentVersion}
						</span>
						<button
							type="button"
							class="action-btn"
							class:success={$applicationUpdateState.status === 'latest'}
							disabled={['checking', 'downloading'].includes(
								$applicationUpdateState.status,
							)}
							onclick={handleUpdateAction}
						>
							{#if $applicationUpdateState.status === 'checking'}
								<span class="loading-spinner"></span>
								<span>{t('settings.checkingUpdates')}</span>
							{:else if $applicationUpdateState.status === 'downloading'}
								<span class="loading-spinner"></span>
								<span>{t('settings.downloadingUpdate')}</span>
							{:else if $applicationUpdateState.status === 'available'}
								<span>
									{t('settings.downloadUpdate', {
										version: $applicationUpdateState.version ?? '',
									})}
								</span>
							{:else if $applicationUpdateState.status === 'latest'}
								<span class="success-text">
									✓ {t('settings.latestVersionInstalled')}
								</span>
							{:else if $applicationUpdateState.status === 'error'}
								<span>{t('common.retry')}</span>
							{:else}
								<span>{t('settings.checkUpdates')}</span>
							{/if}
						</button>
					</div>
				</SettingRow>
			</div>
		</div>

		<!-- Section 2: System & Startup -->
		<div class="settings-section">
			<div class="section-header">
				<span class="section-title">{t('settings.systemSection')}</span>
			</div>
			<div class="settings-group">
				<!-- Auto Start with Windows -->
				<SettingRow label={t('settings.autoStart')}>
					<ToggleSwitch
						mode="checkbox"
						disabled={!runtime.isTauri}
						ariaLabel={t('settings.autoStart')}
						checked={runtime.isTauri ? $nativeSettings.autoStart : false}
						onchange={(val) => updateNativeSettings({ autoStart: val })}
					/>
				</SettingRow>

				<!-- Close to Tray -->
				<SettingRow label={t('settings.closeToTray')}>
					<ToggleSwitch
						mode="checkbox"
						disabled={!runtime.isTauri}
						ariaLabel={t('settings.closeToTray')}
						checked={runtime.isTauri ? $nativeSettings.closeToTray : true}
						onchange={(val) => updateNativeSettings({ closeToTray: val })}
					/>
				</SettingRow>

				<!-- Start Minimized -->
				<SettingRow label={t('settings.startMinimized')}>
					<ToggleSwitch
						mode="checkbox"
						disabled={!runtime.isTauri}
						ariaLabel={t('settings.startMinimized')}
						checked={runtime.isTauri ? $nativeSettings.startMinimized : false}
						onchange={(val) => updateNativeSettings({ startMinimized: val })}
					/>
				</SettingRow>
				<SettingRow
					label={t('settings.notifications')}
					showChevron
					onclick={() => (currentSubView = 'notifications')}
				/>
			</div>
		</div>

		<!-- Section 3: Network / Core Settings -->
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
						value={runtime.isTauri ? $nativeSettings.networkMode : 'proxy'}
						disabled={!runtime.isTauri}
						onChange={(value) =>
							updateNativeSettings({ networkMode: value as 'proxy' | 'tun' })}
						compact
					/>
				</SettingRow>

				<!-- Auto Connect -->
				<SettingRow label={t('settings.autoConnect')}>
					<ToggleSwitch
						mode="checkbox"
						disabled={!runtime.isTauri}
						ariaLabel={t('settings.autoConnect')}
						checked={runtime.isTauri
							? $nativeSettings.autoConnect
							: autoConnect}
						onchange={(value) => updateNativeSettings({ autoConnect: value })}
					/>
				</SettingRow>

				<SettingRow
					label={t('settings.autoReconnect')}
					description={t('settings.autoReconnectDesc')}
				>
					<ToggleSwitch
						mode="checkbox"
						disabled={!runtime.isTauri}
						ariaLabel={t('settings.autoReconnect')}
						checked={$nativeSettings.autoReconnect}
						onchange={(value) => updateNativeSettings({ autoReconnect: value })}
					/>
				</SettingRow>

				<SettingRow
					label={t('settings.serverSelection')}
					description={t('settings.serverSelectionDesc')}
				>
					<Dropdown
						label={t('settings.serverSelection')}
						options={serverSelectionOptions}
						value={$nativeSettings.serverSelection}
						disabled={!runtime.isTauri}
						onChange={(value) =>
							updateNativeSettings({
								serverSelection: value as 'manual' | 'best',
							})}
						compact
					/>
				</SettingRow>

				<!-- DNS Server -->
				<SettingRow
					label={t('settings.dnsServer')}
					description={t('settings.dnsTunOnly')}
				>
					<Dropdown
						label={t('settings.dnsServer')}
						options={dnsOptions}
						value={runtime.isTauri
							? customDnsOpen
								? 'custom'
								: dnsSelection
							: 'system'}
						disabled={!runtime.isTauri}
						onChange={(value) => {
							customDnsOpen = value === 'custom';
							customDnsInvalid = false;
							if (value !== 'custom')
								void updateNativeSettings({ dnsServer: value });
						}}
						compact
					/>
				</SettingRow>
				{#if customDnsOpen || dnsSelection === 'custom'}
					<SettingRow
						label={t('settings.dnsCustomAddress')}
						description={customDnsInvalid
							? t('settings.dnsInvalid')
							: t('settings.dnsCustomHint')}
					>
						<input
							class="ping-url-input custom-dns-input"
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
					</SettingRow>
				{/if}

				<!-- Bypass LAN -->
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

				<!-- Update Interval -->
				<SettingRow label={t('settings.updateInterval')}>
					<Dropdown
						label={t('settings.updateInterval')}
						options={updateIntervalOptions}
						value={runtime.isTauri
							? ($nativeSettings.updateIntervalHours?.toString() ?? '24')
							: '24'}
						disabled={!runtime.isTauri}
						onChange={(value) =>
							updateNativeSettings({
								updateIntervalHours: parseInt(value, 10),
							})}
						compact
					/>
				</SettingRow>
				<SettingRow label={t('settings.pingPolicy')}>
					<Dropdown
						label={t('settings.pingPolicy')}
						options={pingPolicyOptions}
						value={$nativeSettings.pingPolicy}
						disabled={!runtime.isTauri}
						onChange={(value) =>
							updateNativeSettings({
								pingPolicy: value as 'startup' | 'list' | 'manual',
							})}
						compact
					/>
				</SettingRow>

				<!-- Ping Test URL -->
				<SettingRow
					label={t('settings.pingUrl')}
					description={t('settings.pingUrlDesc')}
				>
					<div class="ping-url-wrapper">
						<input
							type="text"
							class="ping-url-input"
							placeholder="http://www.google.com/generate_204"
							value={$nativeSettings.pingUrl || ''}
							onchange={(e) =>
								updateNativeSettings({
									pingUrl: (e.target as HTMLInputElement).value.trim(),
								})}
						/>
					</div>
				</SettingRow>

				<!-- Reset System Proxy -->
				<SettingRow
					label={t('settings.resetProxy')}
					description={t('settings.resetProxyDesc')}
				>
					<button
						type="button"
						class="action-btn"
						class:success={resetSuccess}
						disabled={!runtime.isTauri || resettingProxy}
						onclick={handleResetProxy}
					>
						{resetSuccess
							? t('settings.proxyResetSuccess')
							: i18n.currentLanguage === 'ru'
								? 'Сбросить'
								: 'Reset'}
					</button>
				</SettingRow>

				<!-- Xray Core Status -->
				<SettingRow label={t('settings.xrayCore')}>
					<div class="version-badge-container">
						<span class="badge version-badge">
							{runtime.isTauri
								? $nativeSettings.coreVersion
								: t('settings.mockReady')}
						</span>
						<button
							type="button"
							class="action-btn"
							class:success={$coreUpdateState.status === 'latest'}
							disabled={!runtime.isTauri ||
								['checking', 'installing'].includes($coreUpdateState.status)}
							onclick={handleCoreUpdateAction}
						>
							{#if $coreUpdateState.status === 'checking'}{t(
									'settings.checkingUpdates',
								)}
							{:else if $coreUpdateState.status === 'installing'}{t(
									'settings.downloadingUpdate',
								)}
							{:else if $coreUpdateState.status === 'available'}{t(
									'settings.downloadUpdate',
									{ version: $coreUpdateState.version ?? '' },
								)}
							{:else if $coreUpdateState.status === 'latest'}{t(
									'settings.latestVersionInstalled',
								)}
							{:else if $coreUpdateState.status === 'error'}{t('common.retry')}
							{:else}{t('settings.checkUpdates')}{/if}
						</button>
					</div>
				</SettingRow>
				<!-- GeoIP / GeoSite Database Status -->
				<SettingRow
					label={t('settings.geoDatabases') || 'Базы GeoIP / GeoSite'}
					description={geoError
						? t('errors.' + geoError) ||
							t('settings.geoDataCorrupted') ||
							'Базы GeoIP/GeoSite отсутствуют или повреждены'
						: geoInfo
							? `${geoInfo.source}${geoInfo.lastUpdated ? ' · ' + new Date(geoInfo.lastUpdated).toLocaleDateString() : ''}`
							: undefined}
				>
					<div class="version-badge-container">
						<span
							class="badge version-badge"
							class:error-badge={geoError || (geoInfo && !geoInfo.valid)}
						>
							{geoInfo?.version || '202410152325'}
						</span>
						<button
							type="button"
							class="action-btn"
							class:success={geoUpdateSuccess}
							disabled={geoUpdating}
							onclick={handleUpdateGeo}
						>
							{geoUpdating
								? t('settings.downloadingUpdate') || 'Обновление...'
								: geoUpdateSuccess
									? t('settings.latestVersionInstalled') || 'Обновлено'
									: t('settings.updateDatabases') || 'Обновить базы'}
						</button>
					</div>
				</SettingRow>
				<SettingRow
					label={t('settings.diagnostics')}
					showChevron
					onclick={() => {
						currentSubView = 'diagnostics';
						void loadDiagnostics();
					}}
				/>
				<SettingRow
					label={t('settings.logConsole')}
					description={logConsoleError
						? t('settings.logConsoleError')
						: t('settings.logConsoleDesc')}
				>
					<button
						type="button"
						class="action-btn"
						disabled={!runtime.isTauri || openingLogConsole}
						onclick={() => void openLogConsole()}
					>
						{openingLogConsole
							? t('common.loading')
							: t('settings.openLogConsole')}
					</button>
				</SettingRow>
			</div>
			<p class="availability">
				{runtime.isTauri
					? $settingsError
						? t('errors.' + $settingsError)
						: ''
					: t('demo.unavailable')}
			</p>
		</div>

		<!-- Section 4: Background Shader -->
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
	{:else if currentSubView === 'notifications'}
		<div class="subview-container">
			<div class="subview-header">
				<button
					type="button"
					class="back-btn"
					onclick={() => (currentSubView = 'main')}
					aria-label={t('common.back')}
				>
					<img
						src={chevronIcon}
						alt=""
						class="back-chevron"
						aria-hidden="true"
					/>
					<span class="subview-title">{t('settings.notifications')}</span>
				</button>
			</div>
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
						onchange={(value) => updateNativeSettings({ notifyExpiry: value })}
					/>
				</SettingRow>
				<SettingRow label={t('settings.notifyUpdate')}>
					<ToggleSwitch
						mode="checkbox"
						disabled={!runtime.isTauri}
						ariaLabel={t('settings.notifyUpdate')}
						checked={$nativeSettings.notifyUpdate}
						onchange={(value) => updateNativeSettings({ notifyUpdate: value })}
					/>
				</SettingRow>
			</div>
		</div>
	{:else if currentSubView === 'diagnostics'}
		<div class="subview-container">
			<div class="subview-header">
				<button
					type="button"
					class="back-btn"
					onclick={() => (currentSubView = 'main')}
					aria-label={t('common.back')}
				>
					<img
						src={chevronIcon}
						alt=""
						class="back-chevron"
						aria-hidden="true"
					/>
					<span class="subview-title">{t('settings.diagnostics')}</span>
				</button>
			</div>
			{#if diagnostics}
				<div class="settings-group">
					<SettingRow label={t('settings.diagStatus')}>
						<span class="diag-value">{t('home.' + diagnostics.status)}</span>
					</SettingRow>
					<SettingRow label={t('settings.routingMode')}>
						<span class="diag-value">
							{t(
								diagnostics.networkMode === 'tun'
									? 'settings.modeTun'
									: 'settings.modeProxy',
							)}
						</span>
					</SettingRow>
					<SettingRow label={t('settings.xrayCore')}>
						<span class="diag-value">{diagnostics.coreVersion}</span>
					</SettingRow>
					<SettingRow label="IPv4">
						<span class="diag-value">
							{diagnostics.publicIpv4 ?? t('settings.diagUnavailable')}
						</span>
					</SettingRow>
					<SettingRow label="IPv6">
						<span class="diag-value">
							{diagnostics.publicIpv6 ?? t('settings.diagUnavailable')}
						</span>
					</SettingRow>
					<SettingRow label={t('settings.diagSystemDns')}>
						<span class="diag-value">
							{diagnostics.systemDns.join(', ') ||
								t('settings.diagUnavailable')}
						</span>
					</SettingRow>
					<SettingRow label={t('settings.dnsServer')}>
						<span class="diag-value">{diagnostics.configuredDns}</span>
					</SettingRow>
					<SettingRow label={t('settings.diagTunnel')}>
						<span class="diag-value">
							{diagnostics.tunnelReady
								? t('settings.diagReady')
								: t('settings.diagUnavailable')}
						</span>
					</SettingRow>
					<SettingRow label={t('settings.diagRoutes')}>
						<span class="diag-value">
							{diagnostics.routingConflict
								? t('settings.diagConflict')
								: diagnostics.routesReady
									? t('settings.diagReady')
									: t('settings.diagUnavailable')}
						</span>
					</SettingRow>
					<SettingRow label={t('settings.diagProxy')}>
						<span class="diag-value">
							{diagnostics.proxyManaged
								? t('settings.diagProxyLumen')
								: diagnostics.systemProxyEnabled
									? t('settings.diagProxyOther')
									: t('settings.diagProxyOff')}
						</span>
					</SettingRow>
				</div>
			{/if}
			{#if diagnosticsError}<p class="availability" role="alert">
					{t('settings.diagError')}
				</p>{/if}
			<div class="diagnostics-actions">
				<button
					type="button"
					class="action-btn"
					disabled={diagnosticsBusy || !runtime.isTauri}
					onclick={() => void loadDiagnostics()}
				>
					{diagnosticsBusy
						? t('settings.checkingUpdates')
						: t('settings.diagRefresh')}
				</button>
				<button
					type="button"
					class="action-btn"
					disabled={!diagnostics}
					onclick={() => void copyDiagnostics()}
				>
					{reportCopied ? t('common.copied') : t('settings.diagCopy')}
				</button>
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

	.action-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 5px 12px;
		border-radius: 12px;
		background: rgba(255, 255, 255, 0.08);
		border: 1px solid rgba(255, 255, 255, 0.12);
		color: #ffffff;
		font-family: var(--font-family-base);
		font-size: 12px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.2s ease;
		white-space: nowrap;
	}

	.action-btn:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.16);
		border-color: rgba(255, 255, 255, 0.2);
	}

	.action-btn:active:not(:disabled) {
		transform: scale(0.96);
	}

	.action-btn.success {
		background: rgba(134, 225, 111, 0.2);
		border-color: rgba(134, 225, 111, 0.4);
		color: #86e16f;
	}

	.action-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.version-badge-container {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.version-badge {
		background: rgba(255, 255, 255, 0.1);
		color: rgba(255, 255, 255, 0.85);
		font-variant-numeric: tabular-nums;
	}
	.version-badge.error-badge {
		background: rgba(229, 62, 62, 0.2);
		color: #feb2b2;
	}
	.custom-dns-input {
		width: 130px;
	}
	.diag-value {
		max-width: 160px;
		font-size: 12px;
		color: rgba(255, 255, 255, 0.78);
		text-align: right;
		word-break: break-word;
	}
	.diagnostics-actions {
		display: flex;
		gap: 8px;
		padding: 0 4px;
	}

	.ping-url-wrapper {
		display: flex;
		align-items: center;
		width: 140px;
	}

	.ping-url-input {
		width: 100%;
		background: rgba(0, 0, 0, 0.25);
		border: 1px solid rgba(255, 255, 255, 0.1);
		border-radius: 10px;
		padding: 4px 8px;
		color: #fff;
		font-family: var(--font-family-base);
		font-size: 12px;
		outline: none;
		transition: border-color 0.15s ease;
	}

	.ping-url-input:focus {
		border-color: rgba(255, 255, 255, 0.3);
	}

	.loading-spinner {
		display: inline-block;
		width: 10px;
		height: 10px;
		border: 1.5px solid rgba(255, 255, 255, 0.3);
		border-top-color: #ffffff;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
		margin-right: 4px;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.success-text {
		color: #86e16f;
	}
</style>
