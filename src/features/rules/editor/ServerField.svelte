<script lang="ts">
	import type { Server } from '../../../lib/services/catalog';
	import { t } from '../../../lib/i18n';
	import { serverDisplayName } from '../../../lib/utils/serverDisplayName';
	import CountryFlag from '../../../components/CountryFlag.svelte';
	import DesignPickerField from '../../../components/DesignPickerField.svelte';
	import tunIcon from '../../../assets/icons/mode-tun.svg';
	import blockIcon from '../../../assets/icons/close.svg';
	import syncIcon from '../../../assets/icons/reload.svg';

	interface Props {
		serverId: string;
		server?: Server;
		subscriptionName?: string;
		onSelect: () => void;
	}
	let { serverId, server, subscriptionName, onSelect }: Props = $props();

	const viaSubscription = $derived(
		serverId === 'subscription' ||
			(!!subscriptionName && serverId === subscriptionName),
	);
	const badge = $derived(
		serverId === 'direct'
			? tunIcon
			: serverId === 'block'
				? blockIcon
				: viaSubscription
					? syncIcon
					: '',
	);
	const chosen = $derived(!!badge || !!server);
</script>

<DesignPickerField
	label={t('rules.selectServer')}
	{chosen}
	size="md"
	ariaLabel={t('rules.selectServer')}
	onclick={onSelect}
>
	{#snippet media()}
		{#if badge}
			<span class="server-badge">
				<img src={badge} alt="" width="24" height="24" />
			</span>
		{:else if server}
			<CountryFlag countryCode={server.countryCode} size={32} />
		{/if}
	{/snippet}
	<span class="server-name">
		{#if serverId === 'direct'}
			{t('rules.direct')}
		{:else if serverId === 'block'}
			{t('rules.block')}
		{:else if viaSubscription}
			{t('rules.subscriptionServer')}
		{:else if server}
			{serverDisplayName(server)}
		{/if}
	</span>
</DesignPickerField>

<style>
	.server-badge {
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: #ffffff1a;
	}
	.server-name {
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
