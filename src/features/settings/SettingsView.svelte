<script module lang="ts">
	/** Main-list position kept while the brightness screen replaces settings. */
	let resumeScroll: number | null = null;
</script>

<script lang="ts">
	import backIcon from '../../assets/chevron-left-small.svg';
	import './settings-page.css';
	import { onMount, tick } from 'svelte';
	import SettingRow from '../../components/SettingRow.svelte';
	import ToggleSwitch from '../../components/ToggleSwitch.svelte';
	import { runtime } from '../../lib/runtime';
	import { invoke } from '@tauri-apps/api/core';
	import {
		isAndroid,
		isDesktopWindow,
		supportsCoreUpdate,
		supportsSystemProxy,
		supportsTun,
	} from '../../lib/platform';
	import {
		nativeSettings,
		settingsError,
		updateNativeSettings,
	} from '../../lib/services/settings';
	import {
		i18n,
		t,
		SUPPORTED_LANGUAGES,
		type SupportedLanguage,
	} from '../../lib/i18n';
	import { shaderSettings } from '../../lib/stores/shaderSettings.svelte';
	import { screen } from '../../lib/motion';
	import ChoiceList from './ui/ChoiceList.svelte';
	import SettingHint from './ui/SettingHint.svelte';
	import AppVersionSettings from './pages/AppVersionSettings.svelte';
	import XrayCoreSettings from './pages/XrayCoreSettings.svelte';
	import GeoSettings from './pages/GeoSettings.svelte';
	import DnsSettings from './pages/DnsSettings.svelte';
	import NotificationsSettings from './pages/NotificationsSettings.svelte';
	import ResetProxySettings from './pages/ResetProxySettings.svelte';
	import LogConsoleSettings from './pages/LogConsoleSettings.svelte';
	import PresetsSettings from './pages/PresetsSettings.svelte';
	import ColorsSettings from './pages/ColorsSettings.svelte';

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

	let currentSubView = $state<SubView>('main');
	let scroller = $state<HTMLDivElement | null>(null);
	let mainScroll = 0;

	/** Android: whether Doze may stop the VPN service. */
	let batteryUnrestricted = $state<boolean | null>(null);

	async function readBattery() {
		if (!isAndroid || !runtime.isTauri) return;
		try {
			batteryUnrestricted = await invoke<boolean>('get_battery_optimization');
		} catch {
			batteryUnrestricted = null;
		}
	}

	function requestBattery() {
		void invoke('request_battery_exemption').catch(() => {});
	}

	function openVpnSettings() {
		void invoke('open_vpn_settings').catch(() => {});
	}

	// Android back steps out of a sub-page before leaving settings.
	$effect(() => {
		if (!isAndroid) return;
		const onBack = (event: Event) => {
			if (currentSubView === 'main') return;
			event.preventDefault();
			void back();
		};
		// The exemption dialog returns to Lumen; read the new state then.
		const onVisible = () => {
			if (document.visibilityState === 'visible') void readBattery();
		};
		window.addEventListener('lumen-back', onBack);
		document.addEventListener('visibilitychange', onVisible);
		return () => {
			window.removeEventListener('lumen-back', onBack);
			document.removeEventListener('visibilitychange', onVisible);
		};
	});

	onMount(() => {
		void readBattery();
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

<div class="settings-view" bind:this={scroller}>
	{#key currentSubView}
		<div
			class="settings-page"
			in:screen={{ dir: currentSubView === 'main' ? -1 : 1 }}
		>
			<!-- A sibling of the header, so it stays pinned while the page scrolls. -->
			<button
				type="button"
				class="circle-back"
				aria-label={t('common.back')}
				onclick={() => void back()}
			>
				<img src={backIcon} alt="" width="24" height="24" />
			</button>
			<div class="page-header">
				{#if currentSubView !== 'main'}
					<span class="page-title">{titles[currentSubView]}</span>
				{:else}
					<h1 class="page-title main-title">{t('settings.title')}</h1>
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
						{#if isDesktopWindow}
							<SettingRow label={t('settings.tooltips')}>
								<ToggleSwitch
									mode="checkbox"
									ariaLabel={t('settings.tooltips')}
									checked={shaderSettings.tooltipsEnabled}
									onchange={(val) => shaderSettings.setTooltipsEnabled(val)}
								/>
							</SettingRow>
						{/if}
						{#if isDesktopWindow}
							<SettingRow
								label={t('settings.uiScale')}
								value="{uiScaleValue}%"
								showChevron
								onclick={() => open('uiScale')}
							/>
						{/if}
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
						{#if isDesktopWindow}
							<SettingRow label={t('settings.autoStart')}>
								<ToggleSwitch
									mode="checkbox"
									disabled={!runtime.isTauri}
									ariaLabel={t('settings.autoStart')}
									checked={runtime.isTauri ? $nativeSettings.autoStart : false}
									onchange={(val) => updateNativeSettings({ autoStart: val })}
								/>
							</SettingRow>
						{/if}
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
						{#if isDesktopWindow}
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
						{/if}
						<SettingRow
							label={t('settings.notifications')}
							showChevron
							onclick={() => open('notifications')}
						/>
					</div>
				</section>

				{#if isAndroid}
					<section class="settings-section">
						<h2 class="section-title">{t('settings.androidSection')}</h2>
						<div class="settings-group">
							<SettingRow
								label={t('settings.alwaysOn')}
								showChevron
								onclick={openVpnSettings}
							/>
							<SettingRow
								label={t('settings.battery')}
								value={batteryUnrestricted === null
									? ''
									: t(
											batteryUnrestricted
												? 'settings.batteryOk'
												: 'settings.batteryLimited',
										)}
								showChevron={batteryUnrestricted === false}
								onclick={batteryUnrestricted === false
									? requestBattery
									: undefined}
							/>
						</div>
						<SettingHint
							text={batteryUnrestricted === false
								? t('settings.batteryHint')
								: t('settings.alwaysOnHint')}
						/>
					</section>
				{/if}

				<section class="settings-section">
					<h2 class="section-title">{t('settings.networkSection')}</h2>
					<div class="settings-group">
						{#if supportsSystemProxy}
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
						{/if}
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
						{#if supportsSystemProxy}
							<SettingRow
								label={t('settings.resetProxy')}
								showChevron
								onclick={() => open('resetProxy')}
							/>
						{/if}
						{#if supportsCoreUpdate}
							<SettingRow
								label={t('settings.xrayCore')}
								showChevron
								onclick={() => open('xrayCore')}
							/>
						{/if}
						<SettingRow
							label={t('settings.geoDatabases')}
							showChevron
							onclick={() => open('geo')}
						/>
						{#if isDesktopWindow}
							<SettingRow
								label={t('settings.logConsole')}
								showChevron
								onclick={() => open('logConsole')}
							/>
						{/if}
					</div>
					<SettingHint
						text={runtime.isTauri
							? $settingsError
								? t('errors.' + $settingsError)
								: ''
							: t('demo.unavailable')}
					/>
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
				<ChoiceList
					options={languageOptions}
					value={i18n.currentLanguage}
					onSelect={(value) => i18n.setLanguage(value as SupportedLanguage)}
				/>
			{:else if currentSubView === 'uiScale'}
				<ChoiceList
					options={uiScaleOptions}
					value={uiScaleValue}
					onSelect={(value) => shaderSettings.setUiScale(Number(value) / 100)}
				/>
				<SettingHint text={t('settings.uiScaleDesc')} />
			{:else if currentSubView === 'appVersion'}
				<AppVersionSettings />
			{:else if currentSubView === 'notifications'}
				<NotificationsSettings />
			{:else if currentSubView === 'routingMode'}
				<ChoiceList
					options={routingOptions}
					value={networkMode}
					onSelect={(value) =>
						updateNativeSettings({ networkMode: value as 'proxy' | 'tun' })}
					disabled={!runtime.isTauri}
				/>
				<SettingHint
					text={t(
						supportsTun ? 'settings.routingDesc' : 'settings.routingProxyOnly',
					)}
				/>
			{:else if currentSubView === 'serverSelection'}
				<ChoiceList
					options={serverSelectionOptions}
					value={$nativeSettings.serverSelection}
					onSelect={(value) =>
						updateNativeSettings({
							serverSelection: value as 'manual' | 'best',
						})}
					disabled={!runtime.isTauri}
				/>
				<SettingHint text={t('settings.serverSelectionDesc')} />
			{:else if currentSubView === 'dns'}
				<DnsSettings />
			{:else if currentSubView === 'updateInterval'}
				<ChoiceList
					options={updateIntervalOptions}
					value={updateInterval}
					onSelect={(value) =>
						updateNativeSettings({ updateIntervalHours: parseInt(value, 10) })}
					disabled={!runtime.isTauri}
				/>
			{:else if currentSubView === 'pingPolicy'}
				<ChoiceList
					options={pingPolicyOptions}
					value={$nativeSettings.pingPolicy}
					onSelect={(value) =>
						updateNativeSettings({
							pingPolicy: value as 'startup' | 'list' | 'manual',
						})}
					disabled={!runtime.isTauri}
				/>
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
				<SettingHint text={t('settings.pingUrlDesc')} />
			{:else if currentSubView === 'resetProxy'}
				<ResetProxySettings />
			{:else if currentSubView === 'xrayCore'}
				<XrayCoreSettings />
			{:else if currentSubView === 'geo'}
				<GeoSettings />
			{:else if currentSubView === 'logConsole'}
				<LogConsoleSettings />
			{:else if currentSubView === 'presets'}
				<PresetsSettings />
			{:else if currentSubView === 'colors'}
				<ColorsSettings />
			{/if}
		</div>
	{/key}
</div>
