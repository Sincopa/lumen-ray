<script lang="ts">
	import { onDestroy } from 'svelte';
	import { fade } from 'svelte/transition';
	import { pop, rise } from '../../lib/motion';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import StatusDot from '../../components/StatusDot.svelte';
	import RollingNumber from '../../components/RollingNumber.svelte';
	import { catalog } from '../../lib/services';
	import { errorCode } from '../../lib/services/errors';
	import { t } from '../../lib/i18n';
	import { serverDisplayName } from '../../lib/utils/serverDisplayName';
	import type { Server } from '../../lib/services/catalog';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import reloadIcon from '../../assets/reload.svg';
	import { tabHeader } from '../../app/tabHeader.svelte';

	const catalogStore = catalog.state;
	const pingingServers = catalog.pinging;
	let {
		selectedId = '',
		disabled = false,
		compact = false,
		query = '',
		onSelect,
	}: {
		selectedId?: string;
		disabled?: boolean;
		compact?: boolean;
		/** The compact picker's search; its bar owns the field. */
		query?: string;
		onManageSubscription?: () => void;
		onSelect?: (server: {
			id: string;
			countryCode: string;
			name: string;
		}) => void;
	} = $props();

	let busy = $state(false);
	let error = $state('');
	const subscriptionsById = $derived(
		new Map(
			$catalogStore.subscriptions.map((subscription) => [
				subscription.id,
				subscription,
			]),
		),
	);
	const activeSubscriptionId = $derived(
		$catalogStore.selectedSubscriptionId ??
			$catalogStore.servers.find(
				(server) =>
					server.id === (selectedId || $catalogStore.selectedServerId),
			)?.subscriptionId ??
			$catalogStore.subscriptions[0]?.id,
	);
	let activeSubscription = $derived(
		subscriptionsById.get(activeSubscriptionId ?? '') ??
			$catalogStore.subscriptions[0],
	);
	const subscriptionServers = $derived(
		activeSubscriptionId
			? $catalogStore.servers.filter(
					(server) => server.subscriptionId === activeSubscriptionId,
				)
			: $catalogStore.servers,
	);
	const searchableServers = $derived(
		subscriptionServers.map((server) => ({
			server,
			text: `${serverDisplayName(server)} ${server.protocol} ${subscriptionsById.get(server.subscriptionId)?.name ?? ''}`.toLowerCase(),
		})),
	);
	const normalizedQuery = $derived(
		(compact ? query : tabHeader.query).toLowerCase(),
	);
	let filtered = $derived(
		normalizedQuery
			? searchableServers
					.filter((entry) => entry.text.includes(normalizedQuery))
					.map((entry) => entry.server)
			: subscriptionServers,
	);

	async function refresh() {
		if (busy) return;
		busy = true;
		error = '';
		try {
			await Promise.allSettled(
				subscriptionServers.map((server) =>
					catalog.pingServer(server.id).catch(() => 'timeout'),
				),
			);
		} catch (e) {
			error = t('errors.' + errorCode(e));
		} finally {
			busy = false;
		}
	}

	// The tab shows the shared header; the compact picker keeps its own search.
	const headerOwner = {};
	$effect(() => {
		if (compact) return;
		tabHeader.claim(headerOwner, {
			key: 'servers',
			title: activeSubscription?.name ?? t('servers.title'),
			searchPlaceholder: t('servers.searchPlaceholder') || 'Поиск серверов...',
			actions: headerActions,
		});
	});
	onDestroy(() => tabHeader.release(headerOwner));

	function select(server: Server) {
		onSelect?.({
			id: server.id,
			countryCode: server.countryCode,
			name: serverDisplayName(server),
		});
	}

	function selectSubscription() {
		if (disabled) return;
		onSelect?.({
			id: 'subscription',
			countryCode: '',
			name: activeSubscription?.name ?? t('rules.subscriptionServer'),
		});
	}

	function selectDirect() {
		if (disabled) return;
		onSelect?.({
			id: 'direct',
			countryCode: '',
			name: t('rules.direct'),
		});
	}

	function selectBlock() {
		if (disabled) return;
		onSelect?.({
			id: 'block',
			countryCode: '',
			name: t('rules.block') || 'Блокировать',
		});
	}
</script>

{#snippet headerActions()}
	<IconButton
		design
		icon={reloadIcon}
		ariaLabel={t('servers.reloadServers')}
		disabled={busy || !$catalogStore.servers.length}
		onclick={refresh}
	/>
{/snippet}

<section class="servers-view" class:compact aria-label={t('servers.title')}>
	{#if error}<p role="alert" transition:rise>{error}</p>{/if}

	<div class="servers-list">
		{#if compact}
			<!-- Special Option 1: Active Subscription (Current selected server in this subscription) -->
			<button
				class="server-row special-row"
				class:active-server={selectedId === 'subscription' ||
					(activeSubscription && selectedId === activeSubscription.id)}
				{disabled}
				aria-pressed={selectedId === 'subscription' ||
					(activeSubscription && selectedId === activeSubscription.id)}
				onclick={selectSubscription}
			>
				<span class="special-icon-badge">
					<MaterialIcon name="sync_alt" size={20} />
				</span>
				<span class="server-text">
					<span class="protocol">{t('rules.subscriptionServer')}</span>
					<span class="special-title">
						{activeSubscription?.name ?? 'Подписка'}
					</span>
				</span>
				<span class="ping">
					{#if (selectedId === 'subscription' || (activeSubscription && selectedId === activeSubscription.id)) && !busy}
						<span class="dot-pop" in:pop={{ start: 0.5 }}>
							<StatusDot size={8} />
						</span>
					{/if}
				</span>
			</button>

			<!-- Special Option 2: Direct (Bypass VPN) -->
			<button
				class="server-row special-row"
				class:active-server={selectedId === 'direct'}
				{disabled}
				aria-pressed={selectedId === 'direct'}
				onclick={selectDirect}
			>
				<span class="special-icon-badge direct-badge">
					<MaterialIcon name="alt_route" size={20} />
				</span>
				<span class="server-text">
					<span class="protocol">{t('rules.directDesc')}</span>
					<span class="special-title">{t('rules.direct')}</span>
				</span>
				<span class="ping">
					{#if selectedId === 'direct' && !busy}
						<span class="dot-pop" in:pop={{ start: 0.5 }}>
							<StatusDot size={8} />
						</span>
					{/if}
				</span>
			</button>

			<!-- Special Option 3: Block (Blackhole) -->
			<button
				class="server-row special-row"
				class:active-server={selectedId === 'block'}
				{disabled}
				aria-pressed={selectedId === 'block'}
				onclick={selectBlock}
			>
				<span class="special-icon-badge block-badge">
					<MaterialIcon name="block" size={20} />
				</span>
				<span class="server-text">
					<span class="protocol">{t('rules.blockDesc') || 'Блокировать'}</span>
					<span class="special-title">{t('rules.block')}</span>
				</span>
				<span class="ping">
					{#if selectedId === 'block' && !busy}
						<span class="dot-pop" in:pop={{ start: 0.5 }}>
							<StatusDot size={8} />
						</span>
					{/if}
				</span>
			</button>
		{/if}

		{#each filtered as server (server.id)}
			<button
				class="server-row"
				class:active-server={server.id === selectedId}
				{disabled}
				aria-pressed={server.id === selectedId}
				onclick={() => select(server)}
			>
				<CountryFlag countryCode={server.countryCode} size={32} />
				<span class="server-text">
					<span class="protocol">{server.protocol}</span>
					<span>
						{serverDisplayName(server)}
					</span>
				</span>
				<span class="ping">
					{#if server.id === selectedId && !$pingingServers.has(server.id)}
						<span class="dot-pop" in:pop={{ start: 0.5 }}>
							<StatusDot size={8} />
						</span>
					{/if}
					{#key $pingingServers.has(server.id)}
						<span class="ping-reading" transition:fade={{ duration: 140 }}>
							{#if $pingingServers.has(server.id)}
								<span class="ping-spinner" aria-label="Loading ping"></span>
							{:else if typeof server.latency === 'number'}
								<RollingNumber value={server.latency} /> ms
							{:else if server.latency === 'timeout'}
								{t('servers.timeout')}
							{:else if server.latency}
								{server.latency} ms
							{:else}
								—
							{/if}
						</span>
					{/key}
				</span>
			</button>
		{:else}<p class="hint">{t('servers.empty')}</p>{/each}
	</div>
</section>

<style>
	.dot-pop {
		display: inline-flex;
		flex-shrink: 0;
	}
	.servers-view {
		display: flex;
		flex-direction: column;
		gap: 12px;
		height: 100%;
		min-height: 0;
		position: relative;
	}
	.ping-spinner {
		width: 14px;
		height: 14px;
		border: 2px solid rgba(255, 255, 255, 0.2);
		border-top-color: #ffffff;
		border-radius: 50%;
		animation: ping-spin 0.75s linear infinite;
		display: inline-block;
	}

	.ping-reading {
		display: inline-flex;
		letter-spacing: 0.28px;
		gap: 3px;
		align-items: center;
		justify-content: flex-end;
		min-width: 42px;
	}

	@keyframes ping-spin {
		to {
			transform: rotate(360deg);
		}
	}
	.servers-list {
		overflow-y: auto;
		min-height: 0;
		flex: 1;
		padding-bottom: 96px;
		scrollbar-width: none;
		-ms-overflow-style: none;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.servers-list::-webkit-scrollbar {
		display: none;
	}
	.server-row {
		width: 100%;
		min-height: var(--design-row-height);
		height: var(--design-row-height);
		flex-shrink: 0;
		display: flex;
		gap: 6px;
		align-items: center;
		border: none;
		border-radius: 123px;
		background: transparent;
		padding: 12px 26px 12px 12px;
		text-align: left;
		transition:
			background-color 180ms ease,
			transform 180ms var(--ease-spring);
	}
	.server-row:hover,
	.server-row.active-server {
		background: var(--design-panel);
		border-color: transparent;
	}
	.server-row:active {
		transform: scale(0.985);
	}
	.server-text {
		display: flex;
		flex-direction: column;
		gap: 6px;
		min-width: 0;
		flex: 1;
		overflow-wrap: anywhere;
		font-size: 14px;
		line-height: 14px;
	}
	.protocol {
		font-size: 10px;
		line-height: 1;
		font-weight: 500;
		letter-spacing: 0.4px;
		color: #ffffff80;
		text-transform: uppercase;
	}
	.ping {
		font-size: 14px;
		font-weight: 500;
		white-space: nowrap;
		color: #fff;
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.special-icon-badge {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.1);
		display: flex;
		align-items: center;
		justify-content: center;
		color: #ffffff;
		flex-shrink: 0;
	}
	.special-icon-badge.direct-badge {
		background: rgba(134, 225, 111, 0.15);
		color: #86e16f;
	}
	.special-icon-badge.block-badge {
		background: rgba(255, 99, 132, 0.15);
		color: #ff6384;
	}
	.special-title {
		font-size: 14px;
		font-weight: 500;
		color: #ffffff;
	}
	.hint {
		font-size: 12px;
		color: #d0c4dc;
		margin: 4px 0;
	}
</style>
