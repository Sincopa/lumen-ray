<script lang="ts">
	import { onDestroy, tick } from 'svelte';
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
	import reloadIcon from '../../assets/icons/reload.svg';
	import moreIcon from '../../assets/icons/more.svg';
	import spinnerIcon from '../../assets/icons/spinner.svg';
	import tunIcon from '../../assets/icons/mode-tun.svg';
	import blockIcon from '../../assets/icons/close.svg';
	import { tabHeader } from '../../app/tabHeader.svelte';

	const catalogStore = catalog.state;
	const pingingServers = catalog.pinging;

	// Figma "альтернативная сетка": the tab can show servers as a two-column grid.
	const layoutKey = 'lumen.serversLayout';
	function loadGrid(): boolean {
		try {
			return localStorage.getItem(layoutKey) === 'grid';
		} catch {
			return false;
		}
	}
	let grid = $state(loadGrid());
	let morphing = $state(false);
	function toggleLayout() {
		const apply = () => {
			grid = !grid;
			try {
				localStorage.setItem(layoutKey, grid ? 'grid' : 'list');
			} catch {
				/* Storage refused; the choice lasts for this session. */
			}
		};
		// The browser morphs each named card from its old box to its new one.
		const doc = document as Document & {
			startViewTransition?: (update: () => Promise<void>) => {
				finished: Promise<void>;
			};
		};
		if (
			!doc.startViewTransition ||
			matchMedia('(prefers-reduced-motion: reduce)').matches
		) {
			apply();
			return;
		}
		morphing = true;
		void tick().then(() => {
			const transition = doc.startViewTransition!(async () => {
				apply();
				await tick();
			});
			void transition.finished.finally(() => (morphing = false));
		});
	}
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
		class={busy ? 'servers-refresh refreshing' : 'servers-refresh'}
		onclick={refresh}
	/>
	<IconButton
		design
		icon={moreIcon}
		ariaLabel={t(grid ? 'servers.showList' : 'servers.showGrid')}
		active={grid}
		onclick={toggleLayout}
	/>
{/snippet}

<section class="servers-view" class:compact aria-label={t('servers.title')}>
	{#if error}<p role="alert" transition:rise>{error}</p>{/if}

	<div class="servers-list" class:grid={grid && !compact}>
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
					<img src={reloadIcon} alt="" width="24" height="24" />
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
					<img src={tunIcon} alt="" width="24" height="24" />
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
					<img src={blockIcon} alt="" width="24" height="24" />
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

		{#each filtered as server, index (server.id)}
			<button
				class="server-row"
				style:view-transition-name={morphing ? `server-card-${index}` : null}
				class:active-server={server.id === selectedId}
				{disabled}
				aria-pressed={server.id === selectedId}
				onclick={() => select(server)}
			>
				<span class="server-head">
					<CountryFlag
						countryCode={server.countryCode}
						size={grid && !compact ? 24 : 32}
					/>
					<span class="server-text">
						<span class="protocol">{server.protocol}</span>
						<span class="server-name">
							{serverDisplayName(server)}
						</span>
					</span>
				</span>
				<span class="ping">
					{#if server.id === selectedId && !$pingingServers.has(server.id)}
						<span class="dot-pop" in:pop={{ start: 0.5 }}>
							<StatusDot size={8} />
						</span>
					{/if}
					{#key $pingingServers.has(server.id)}
						<span
							class="ping-reading"
							class:muted={server.latency === 'timeout'}
							transition:fade={{ duration: 140 }}
						>
							{#if $pingingServers.has(server.id)}
								<img
									class="ping-spinner"
									src={spinnerIcon}
									alt=""
									width="16"
									height="16"
									aria-label={t('servers.pinging')}
								/>
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
	/* The tab shows the shared header, which floats over the top. */
	.servers-view:not(.compact) {
		padding-top: var(--tab-header-space);
	}
	/* Figma "region 2 loading": a 16px open ring turning in a 24px box. */
	.ping-spinner {
		display: block;
		margin: 4px;
		animation: ping-spin 0.9s linear infinite;
	}

	.ping-reading {
		display: inline-flex;
		letter-spacing: 0.28px;
		gap: 3px;
		align-items: center;
		justify-content: flex-end;
		min-width: 42px;
	}
	/* "timeout" is regular weight at 50%. */
	.ping-reading.muted {
		font-weight: 400;
		color: #ffffff80;
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
	.compact .servers-list {
		padding-top: 16px;
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
		border-radius: 24px;
		background: transparent;
		padding: 12px 26px 12px 12px;
		text-align: left;
		transition:
			background-color 180ms ease,
			transform 180ms var(--ease-spring);
	}
	.server-row:hover {
		background-color: var(--design-panel);
	}
	/* The current server: 2% white under a 5% green tint, flattened into one
	   colour so the highlight can fade between rows. */
	.server-row.active-server {
		background-color: rgb(112 243 100 / 0.07);
	}
	.server-row:active {
		transform: scale(0.985);
	}
	.server-head {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
		flex: 1;
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
		letter-spacing: 0.28px;
		white-space: nowrap;
		color: #fff;
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	/* Two columns of 18px cards: flag and name on top, the ping at the bottom. */
	.servers-list.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		align-content: start;
		gap: 12px;
	}
	.grid .server-row {
		flex-direction: column;
		align-items: flex-start;
		justify-content: space-between;
		height: 99px;
		min-height: 99px;
		padding: 16px;
		border-radius: 18px;
		background-color: var(--design-panel);
	}
	.grid .server-row.active-server {
		background-color: rgb(112 243 100 / 0.07);
	}
	.grid .server-head {
		flex: 0 0 auto;
		align-items: flex-end;
		gap: 10px;
		width: 100%;
	}
	.grid .server-text {
		gap: 7px;
	}
	.grid .server-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* The dot follows the ping in a card; the ping is at 75% until chosen. */
	.grid .ping {
		flex-direction: row-reverse;
		color: #ffffffbf;
	}
	.grid .active-server .ping {
		color: #fff;
	}
	.grid .ping-reading {
		justify-content: flex-start;
		min-width: 0;
	}
	@media (hover: hover) and (pointer: fine) {
		.grid .server-row:hover:not(.active-server) {
			background-color: #ffffff0a;
		}
	}
	/* The cards morph on the default spring; their contents cross-fade without
	   stretching, clipped to the moving box. */
	:global(::view-transition-group(*)) {
		animation-duration: 520ms;
		animation-timing-function: var(--ease-morph);
	}
	:global(::view-transition-image-pair(*)) {
		overflow: clip;
	}
	:global(::view-transition-old(*)),
	:global(::view-transition-new(*)) {
		width: auto;
		height: auto;
		object-fit: none;
		object-position: left top;
		animation-duration: 280ms;
		animation-timing-function: ease;
	}
	:global(::view-transition-group(root)) {
		animation-duration: 280ms;
	}
	:global(.servers-refresh.refreshing .btn-icon) {
		animation: ping-spin 900ms linear infinite;
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
