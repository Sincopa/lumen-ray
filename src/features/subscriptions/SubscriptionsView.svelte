<script lang="ts">
	import { onDestroy } from 'svelte';
	import { get } from 'svelte/store';
	import { SvelteSet } from 'svelte/reactivity';
	import { createDeferredTask } from '../../lib/utils/deferredTask';
	import { flip } from 'svelte/animate';
	import {
		reveal,
		rise,
		pop,
		swap,
		easeOut,
		morphGrow,
	} from '../../lib/motion';
	import { catalog, connection } from '../../lib/services';
	import { errorCode } from '../../lib/services/errors';
	import { canSwitch, isActive } from '../../lib/services/connection';
	import { refreshSubscriptionWithStatus } from '../../lib/services/maintenance';
	import { t } from '../../lib/i18n';
	import type { Subscription } from '../../lib/services/catalog';
	import ConfirmDialog from '../../components/ConfirmDialog.svelte';
	import PopupMenu from '../../components/PopupMenu.svelte';
	import StatusDot from '../../components/StatusDot.svelte';
	import SubscriptionRenameField from './parts/SubscriptionRenameField.svelte';
	import SubscriptionCardActions from './parts/SubscriptionCardActions.svelte';
	import SubscriptionDetails from './parts/SubscriptionDetails.svelte';
	import SubscriptionMenuItems from './parts/SubscriptionMenuItems.svelte';
	import SubscriptionShareDialog from './parts/SubscriptionShareDialog.svelte';

	const focusTask = createDeferredTask();
	onDestroy(() => focusTask.cancel());

	const catalogStore = catalog.state;
	const connectionStore = connection.state;

	// Local state
	let editingId = $state<string | null>(null);
	let editName = $state('');
	let editInputEl = $state<HTMLInputElement | null>(null);
	let deleting = $state<Subscription | null>(null);
	let menu = $state<string | null>(null);
	// Each card refreshes on its own; one refresh does not lock the others.
	const refreshing = new SvelteSet<string>();
	let error = $state('');
	let deleteBusy = $state(false);
	let sharing = $state<Subscription | null>(null);
	let sharedSource = $state('');
	let shareError = $state('');

	let activeSubId = $derived(
		$catalogStore.selectedSubscriptionId ??
			$catalogStore.servers.find((s) => s.id === $catalogStore.selectedServerId)
				?.subscriptionId ??
			$catalogStore.subscriptions[0]?.id,
	);

	// A subscription can go while connected; only a connect or disconnect in
	// flight waits, so the removal does not race the tunnel it may stop.
	const deleteBlocked = $derived($connectionStore.busy);
	/** The tunnel runs (or is heading) through a server of `sub`. */
	function carriesTunnel(sub: Subscription) {
		const snapshot = $connectionStore;
		if (!isActive(snapshot.status)) return false;
		const serverId = snapshot.info?.serverId ?? snapshot.target;
		return $catalogStore.servers.some(
			(server) => server.id === serverId && server.subscriptionId === sub.id,
		);
	}

	let menuTop = $state(68);
	let viewEl = $state<HTMLElement | null>(null);

	let activeMenuSub = $derived(
		$catalogStore.subscriptions.find((s) => s.id === menu),
	);
	async function activateSubscription(id: string) {
		try {
			await catalog.selectSubscription(id);
			// The subscription brings back the server last chosen in it. A running
			// tunnel moves there too instead of staying on the old subscription.
			const serverId = get(catalog.state).selectedServerId;
			const snapshot = get(connection.state);
			if (
				serverId &&
				isActive(snapshot.status) &&
				canSwitch(snapshot.status) &&
				snapshot.info?.serverId !== serverId
			)
				await connection.switchServer(serverId);
		} catch (cause) {
			error = t('errors.' + errorCode(cause));
		}
	}

	function selectSubscription(id: string) {
		if (activeSubId !== id) void activateSubscription(id);
	}

	/** The whole card selects it; its own buttons and fields keep their clicks. */
	function selectFromCard(id: string, event: MouseEvent) {
		const target = event.target as HTMLElement | null;
		if (target?.closest('button, input, a, .popup-menu')) return;
		if (editingId === id) return;
		selectSubscription(id);
	}

	function toggleMenu(subId: string, e: MouseEvent) {
		if (menu === subId) {
			menu = null;
			return;
		}
		const btn = e.currentTarget as HTMLElement | null;
		if (btn && viewEl) {
			const btnRect = btn.getBoundingClientRect();
			const viewRect = viewEl.getBoundingClientRect();
			menuTop = Math.round(btnRect.bottom - viewRect.top + 8);
		}
		menu = subId;
	}

	// Action handlers
	async function refresh(id: string) {
		if (refreshing.has(id)) return;
		refreshing.add(id);
		error = '';
		try {
			await refreshSubscriptionWithStatus(id);
		} catch (e) {
			error = t('errors.' + errorCode(e));
		} finally {
			refreshing.delete(id);
		}
	}

	function startEditing(sub: Subscription) {
		editingId = sub.id;
		editName = sub.name;
		menu = null;
		focusTask.schedule(() => {
			editInputEl?.focus();
			editInputEl?.select();
		}, 100);
	}

	function cancelEditing() {
		focusTask.cancel();
		editingId = null;
		editName = '';
	}

	async function saveEditing(subId: string) {
		focusTask.cancel();
		const trimmed = editName.trim();
		if (!trimmed) return;
		try {
			await catalog.renameSubscription(subId, trimmed);
			editingId = null;
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	function requestDelete(sub: Subscription) {
		deleting = sub;
		menu = null;
	}

	async function remove() {
		if (!deleting) return;
		deleteBusy = true;
		try {
			await catalog.removeSubscription(deleting.id);
			deleting = null;
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		} finally {
			deleteBusy = false;
		}
	}

	async function showSubscriptionSource(sub: Subscription) {
		menu = null;
		sharing = sub;
		sharedSource = '';
		shareError = '';
		try {
			sharedSource = await catalog.getSubscriptionSource(sub.id);
		} catch (cause) {
			shareError = t('subscriptions.sourceUnavailable');
			console.error('[subscription-source] Unable to retrieve source', cause);
		}
	}
</script>

<svelte:window
	onclick={(e) => {
		if (menu) {
			const target = e.target as HTMLElement | null;
			if (!target?.closest('.popup-menu') && !target?.closest('.icon-btn')) {
				menu = null;
			}
		}
	}}
/>

<!-- Figma "Подписки": every profile is a card with its numbers; the one in use
     is lighter and carries the green dot. -->
<section
	class="subscriptions-view"
	bind:this={viewEl}
	aria-label={t('subscriptions.title')}
>
	{#if error}<p role="alert" transition:rise>{error}</p>{/if}
	<div class="cards-list" onscroll={() => (menu = null)}>
		{#each $catalogStore.subscriptions as sub (sub.id)}
			{@const active = sub.id === activeSubId}
			{@const editing = editingId === sub.id}
			<div
				class="sub-card-wrapper"
				class:has-open-menu={menu === sub.id}
				animate:flip={{ duration: 280, easing: easeOut }}
				in:pop={{ start: 0.96, duration: 220 }}
				out:pop={{ start: 0.96, duration: 180 }}
			>
				<article
					class="sub-card"
					class:active
					class:editing
					role="presentation"
					onclick={(e) => selectFromCard(sub.id, e)}
				>
					<div class="card-header">
						{#if editing}
							<div class="rename-wrap" in:morphGrow>
								<SubscriptionRenameField
									bind:name={editName}
									bind:inputEl={editInputEl}
									onSave={() => saveEditing(sub.id)}
									onCancel={cancelEditing}
								/>
							</div>
						{:else}
							<button
								in:swap
								class="title-group"
								aria-pressed={active}
								onclick={() => selectSubscription(sub.id)}
							>
								<span class="sub-name">{sub.name}</span>
								{#if active}
									<span class="dot" in:pop={{ start: 0.5 }}>
										<StatusDot />
									</span>
								{/if}
							</button>
							<div class="actions-group">
								<SubscriptionCardActions
									busy={refreshing.has(sub.id)}
									menuOpen={menu === sub.id}
									onRefresh={() => refresh(sub.id)}
									onToggleMenu={(e) => toggleMenu(sub.id, e)}
								/>
							</div>
						{/if}
					</div>
					{#if !editing}
						<div class="card-body" transition:reveal>
							<SubscriptionDetails {sub} />
						</div>
					{/if}
				</article>
			</div>
		{/each}
		{#if !$catalogStore.subscriptions.length}
			<p class="empty">{t('subscriptions.empty')}</p>
		{/if}
	</div>

	{#if menu && activeMenuSub}
		<PopupMenu top="{menuTop}px" right="20px">
			<SubscriptionMenuItems
				sub={activeMenuSub}
				deleteDisabled={deleteBlocked}
				onEdit={startEditing}
				onShare={showSubscriptionSource}
				onDelete={requestDelete}
			/>
		</PopupMenu>
	{/if}
</section>

{#if deleting}
	<ConfirmDialog
		title={t('common.areYouSure')}
		description={t('subscriptions.deleteConfirm', { name: deleting.name }) +
			(carriesTunnel(deleting)
				? ' ' + t('subscriptions.deleteDisconnects')
				: '')}
		cancelLabel={t('common.cancel')}
		confirmLabel={t('common.continue')}
		onCancel={() => (deleting = null)}
		onConfirm={remove}
		busy={deleteBusy}
		destructive
	/>
{/if}

{#if sharing}
	<SubscriptionShareDialog
		subscription={sharing}
		source={sharedSource}
		error={shareError}
		onClose={() => (sharing = null)}
	/>
{/if}

<style>
	.subscriptions-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		min-height: 0;
		gap: 12px;
		padding-top: 0;
		position: relative;
	}
	.cards-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 100%;
		overflow-y: auto;
		min-height: 0;
		flex: 1;
		padding-bottom: 96px;
		scrollbar-width: none;
		-ms-overflow-style: none;
	}
	.cards-list::-webkit-scrollbar {
		display: none;
	}
	.sub-card-wrapper {
		position: relative;
		width: 100%;
		flex-shrink: 0;
	}
	.sub-card-wrapper.has-open-menu {
		z-index: 50;
	}
	/* 24px cards, 19/20/19/23 padding and 20px between the title row and the
	   numbers. 2% white; the active profile 5% with a soft inner glow. */
	.sub-card {
		display: flex;
		flex-direction: column;
		gap: 20px;
		min-height: 66px;
		padding: 19px 20px 19px 23px;
		border-radius: 24px;
		background: var(--design-panel);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transition:
			background-color 200ms ease,
			box-shadow 200ms ease,
			gap 220ms ease;
		cursor: pointer;
	}
	.sub-card.active {
		background: var(--design-panel-open);
		box-shadow: var(--design-glow);
	}
	.sub-card.editing {
		gap: 0;
		cursor: default;
	}
	@media (hover: hover) and (pointer: fine) {
		.sub-card:not(.active):hover {
			background: #ffffff0a;
		}
	}
	.card-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		height: 28px;
		gap: 8px;
	}
	.rename-wrap {
		display: flex;
		align-items: center;
		width: 100%;
		height: 100%;
	}
	.title-group {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		min-height: 0;
		background: none;
		border: 0;
		padding: 0;
		text-align: left;
		flex: 1;
	}
	/* Not cap-trimmed: the ellipsis clip would cut ascenders and descenders. */
	.sub-name {
		font-size: 16px;
		font-weight: 400;
		line-height: 1.3;
		letter-spacing: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dot {
		display: inline-flex;
		flex-shrink: 0;
	}
	.actions-group {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.card-body {
		width: 100%;
		text-align: left;
		cursor: pointer;
		display: flex;
		flex-direction: column;
	}
	.empty {
		color: #ffffff40;
		font-size: 14px;
		text-align: center;
	}
</style>
