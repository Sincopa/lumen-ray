<script lang="ts">
	import { onDestroy } from 'svelte';
	import { fade } from 'svelte/transition';
	import { pop, rise } from '../../lib/motion';
	import { createDeferredTask } from '../../lib/utils/deferredTask';
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
	import searchIcon from '../../assets/search.svg';
	import crossIcon from '../../assets/close.svg';
	import { tabHeader } from '../../app/tabHeader.svelte';

	const searchTask = createDeferredTask();
	onDestroy(() => {
		searchTask.cancel();
	});
	const catalogStore = catalog.state;
	const pingingServers = catalog.pinging;
	let {
		selectedId = '',
		disabled = false,
		compact = false,
		searchSignal = 0,
		onSelect,
	}: {
		selectedId?: string;
		disabled?: boolean;
		compact?: boolean;
		searchSignal?: number;
		onManageSubscription?: () => void;
		onSelect?: (server: {
			id: string;
			countryCode: string;
			name: string;
		}) => void;
	} = $props();

	let query = $state('');
	let searching = $state(false);
	let searchClosing = $state(false);
	let searchInputEl = $state<HTMLInputElement | null>(null);
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

	function handleOpenSearch() {
		searching = true;
		searchClosing = false;
		searchTask.schedule(() => {
			searchInputEl?.focus();
		}, 180);
	}

	function handleCloseSearch() {
		searchClosing = true;
		searchTask.schedule(() => {
			searching = false;
			searchClosing = false;
			query = '';
		}, 240);
	}

	$effect(() => {
		if (searchSignal > 0) handleOpenSearch();
	});

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

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape' && searching) handleCloseSearch();
	}}
/>

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
	{#if compact}
		<div class="header-wrapper" class:open={searching}>
			<div class="header-pill">
				<!-- Standard header content -->
				<div class="header-normal" class:header-hidden={searching}>
					<div class="header-card-title">
						<span class="title-text">
							{activeSubscription?.name ?? t('servers.title')}
						</span>
						<StatusDot variant="active" />
					</div>
					<div class="header-card-actions">
						<IconButton
							design
							icon={searchIcon}
							ariaLabel={t('common.search')}
							onclick={handleOpenSearch}
						/>
						<IconButton
							design
							icon={reloadIcon}
							ariaLabel={t('servers.reloadServers')}
							disabled={busy || !$catalogStore.servers.length}
							onclick={refresh}
						/>
					</div>
				</div>

				<!-- Search bar inside header pill -->
				<div
					class="header-search"
					class:search-open={searching && !searchClosing}
					class:search-closing={searchClosing}
				>
					<img src={searchIcon} alt="" class="search-header-icon" />
					<input
						bind:this={searchInputEl}
						type="text"
						class="search-header-input"
						placeholder={t('servers.searchPlaceholder') || 'Поиск серверов...'}
						bind:value={query}
						autocomplete="off"
						spellcheck="false"
					/>
					<button
						type="button"
						class="close-search-btn"
						onclick={handleCloseSearch}
						aria-label={t('common.close')}
					>
						<img src={crossIcon} alt="" class="close-icon" />
					</button>
				</div>
			</div>
		</div>
	{/if}

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
	/* The compact picker is a bare list. Search reuses the header. */
	.compact .header-wrapper:not(.open) {
		display: none;
	}
	.servers-view {
		display: flex;
		flex-direction: column;
		gap: 12px;
		height: 100%;
		min-height: 0;
		position: relative;
	}
	.header-wrapper {
		position: relative;
		flex-shrink: 0;
	}
	.header-pill {
		position: relative;
		height: var(--design-header-height);
		background: var(--design-panel);
		border-radius: 124px;
		-webkit-backdrop-filter: blur(14px);
		backdrop-filter: blur(14px);
		box-sizing: border-box;
		width: 100%;
		overflow: hidden;
		border: 0;
		transition: border-color 180ms ease;
	}

	.header-pill:focus-within {
		border-color: rgba(255, 255, 255, 0.35);
	}

	.header-normal {
		width: 100%;
		height: 100%;
		padding: 0 20px 0 23px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		box-sizing: border-box;
		gap: 8px;
		transition:
			opacity 200ms ease,
			transform 200ms ease;
	}

	.header-normal.header-hidden {
		opacity: 0;
		pointer-events: none;
		transform: scale(0.96);
	}

	.header-card-title {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		font-size: 16px;
		font-weight: 500;
		letter-spacing: 0;
	}

	.title-text {
		line-height: 1.3;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.header-card-actions {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-shrink: 0;
	}

	/* Header Search Bar */
	.header-search {
		position: absolute;
		left: 0;
		top: 0;
		width: 100%;
		height: 100%;
		padding: 0 8px 0 20px;
		display: flex;
		align-items: center;
		gap: 12px;
		box-sizing: border-box;
		opacity: 0;
		pointer-events: none;
		transform: scale(0.96);
		transition:
			opacity 220ms ease,
			transform 220ms cubic-bezier(0.22, 1, 0.36, 1);
	}

	.header-search.search-open {
		opacity: 1;
		pointer-events: auto;
		transform: scale(1);
	}

	.header-search.search-closing {
		opacity: 0;
		pointer-events: none;
		transform: scale(0.96);
	}

	.search-header-icon {
		width: 28px;
		height: 28px;
		object-fit: contain;
		filter: brightness(0) invert(1);
		opacity: 0.6;
		flex-shrink: 0;
	}

	.search-header-input {
		flex: 1;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		border-radius: 0 !important;
		color: #ffffff;
		font-size: 14px;
		font-family: inherit;
		padding: 0 !important;
		-webkit-appearance: none;
		appearance: none;
	}

	.search-header-input::placeholder {
		color: rgba(255, 255, 255, 0.4);
	}

	.close-search-btn {
		width: 36px;
		height: 36px;
		border-radius: 50%;
		background: #ffffff;
		border: none !important;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0;
		flex-shrink: 0;
		transition: transform 160ms var(--ease-spring);
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
	}

	.close-search-btn:hover {
		transform: scale(1.06);
	}

	.close-search-btn:active {
		transform: scale(0.94);
	}

	.close-icon {
		width: 14px;
		height: 14px;
		object-fit: contain;
		filter: brightness(0);
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
		gap: 6px;
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
		border-radius: 30px;
		background: transparent;
		padding: 12px 27px 12px 12px;
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
		letter-spacing: 0.5px;
		color: #ffffff70;
		text-transform: uppercase;
	}
	.ping {
		font-size: 14px;
		font-weight: 500;
		white-space: nowrap;
		color: #ffffffe6;
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
