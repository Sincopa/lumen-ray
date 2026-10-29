<script lang="ts">
	import { runtime } from '../../../lib/runtime';
	import { isAndroid, supportsTun } from '../../../lib/platform';
	import {
		nativeSettings,
		updateNativeSettings,
	} from '../../../lib/services/settings';
	import { t } from '../../../lib/i18n';
	import { reveal } from '../../../lib/motion';
	import ChoiceList from '../ui/ChoiceList.svelte';
	import SettingHint from '../ui/SettingHint.svelte';

	const presetServers = [
		'system',
		'1.1.1.1',
		'8.8.8.8',
		'94.140.14.14',
		'9.9.9.9',
	];

	let customOpen = $state(false);
	let customDraft = $state('');
	let customInvalid = $state(false);

	let options = $derived([
		{ value: 'system', label: t('settings.dnsSystem') },
		{ value: '1.1.1.1', label: t('settings.dnsCloudflare') },
		{ value: '8.8.8.8', label: t('settings.dnsGoogle') },
		{ value: '94.140.14.14', label: t('settings.dnsAdGuard') },
		{ value: '9.9.9.9', label: t('settings.dnsQuad9') },
		{ value: 'custom', label: t('settings.dnsCustom') },
	]);
	let selection = $derived(
		presetServers.includes($nativeSettings.dnsServer)
			? $nativeSettings.dnsServer
			: 'custom',
	);
	let shownValue = $derived(
		runtime.isTauri ? (customOpen ? 'custom' : selection) : 'system',
	);

	$effect(() => {
		if (selection === 'custom' && !customOpen) {
			customDraft = $nativeSettings.dnsServer;
		}
	});

	function select(value: string) {
		customOpen = value === 'custom';
		customInvalid = false;
		if (value !== 'custom') void updateNativeSettings({ dnsServer: value });
	}

	function commitCustom() {
		const value = customDraft.trim();
		const octets = value.split('.');
		const valid =
			octets.length === 4 &&
			octets.every(
				(octet) => /^\d{1,3}$/.test(octet) && Number(octet) <= 255,
			) &&
			value !== '0.0.0.0' &&
			value !== '255.255.255.255' &&
			Number(octets[0]) < 224;
		customInvalid = !valid;
		if (valid) void updateNativeSettings({ dnsServer: value });
	}
</script>

<ChoiceList
	{options}
	value={shownValue}
	onSelect={select}
	disabled={!runtime.isTauri}
/>
{#if customOpen || selection === 'custom'}
	<div class="settings-group field-group" transition:reveal>
		<input
			class="text-field"
			type="text"
			inputmode="decimal"
			placeholder="9.9.9.9"
			aria-label={t('settings.dnsCustomAddress')}
			bind:value={customDraft}
			onchange={commitCustom}
			onkeydown={(event) => {
				if (event.key === 'Enter') (event.target as HTMLInputElement).blur();
			}}
		/>
	</div>
	<SettingHint
		text={customInvalid
			? t('settings.dnsInvalid')
			: t('settings.dnsCustomHint')}
		alert={customInvalid}
	/>
{/if}
<!-- Android always tunnels, so the setting always applies there. -->
<SettingHint
	text={supportsTun && !isAndroid ? t('settings.dnsTunOnly') : undefined}
/>
