<script lang="ts">
	import chevronIcon from '../../../assets/chevron-down.svg';
	import type { Server } from '../../../lib/services/catalog';
	import { t } from '../../../lib/i18n';
	import { serverDisplayName } from '../../../lib/utils/serverDisplayName';
	import CountryFlag from '../../../components/CountryFlag.svelte';
	import MaterialIcon from '../../../components/MaterialIcon.svelte';

	interface Props {
		serverId: string;
		server?: Server;
		subscriptionName?: string;
		onSelect: () => void;
	}
	let { serverId, server, subscriptionName, onSelect }: Props = $props();

	const chosen = $derived(
		serverId === 'direct' ||
			serverId === 'block' ||
			serverId === 'subscription' ||
			(!!subscriptionName && serverId === subscriptionName) ||
			!!server,
	);
</script>

<div class="field-group">
	<span class="field-label">{t('rules.server')}</span>
	<button
		type="button"
		class="selector-pill"
		class:chosen
		onclick={onSelect}
		aria-label={t('rules.selectServer')}
	>
		<span class="server-info">
			{#if serverId === 'direct'}
				<span class="server-badge">
					<MaterialIcon name="alt_route" size={20} />
				</span>
				<span class="selector-value">{t('rules.direct')}</span>
			{:else if serverId === 'block'}
				<span class="server-badge">
					<MaterialIcon name="block" size={20} />
				</span>
				<span class="selector-value">{t('rules.block')}</span>
			{:else if serverId === 'subscription' || (subscriptionName && serverId === subscriptionName)}
				<span class="server-badge">
					<MaterialIcon name="sync_alt" size={20} />
				</span>
				<span class="selector-value">
					{subscriptionName ?? t('rules.subscriptionServer')}
				</span>
			{:else if server}
				<CountryFlag countryCode={server.countryCode} size={32} />
				<span class="selector-value">{serverDisplayName(server)}</span>
			{:else}
				<span class="selector-value placeholder-val">
					{t('rules.selectServer')}
				</span>
			{/if}
		</span>
		<img src={chevronIcon} alt="" class="chevron-right" />
	</button>
</div>
