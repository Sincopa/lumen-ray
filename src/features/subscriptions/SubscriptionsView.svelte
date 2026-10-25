<script lang="ts">
	import { onDestroy } from 'svelte';
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
	import { refreshSubscriptionWithStatus } from '../../lib/services/maintenance';
	import { t } from '../../lib/i18n';
	import type { Subscription } from '../../lib/services/catalog';
	import ConfirmDialog from '../../components/ConfirmDialog.svelte';
	import PopupMenu from '../../components/PopupMenu.svelte';
	import { tabHeader } from '../../app/tabHeader.svelte';
	import { loadExpandedIds, saveExpandedIds } from './subscriptionExpansion';
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
	const expanded = new SvelteSet<string>(loadExpandedIds());
	let editingId = $state<string | null>(null);
	let editName = $state('');
	let editInputEl = $state<HTMLInputElement | null>(null);
	let deleting = $state<Subscription | null>(null);
	let menu = $state<string | null>(null);
	let busy = $state<string | null>(null);
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

	const activeSub = $derived(
		$catalogStore.subscriptions.find((sub) => sub.id === activeSubId),
	);
	// The active subscription is pinned in the shared header above the list.
	const otherSubs = $derived(
		$catalogStore.subscriptions.filter((sub) => sub.id !== activeSub?.id),
	);
	const deleteBlocked = $derived(
		$connectionStore.busy || $connectionStore.status === 'connected',
	);

	const headerOwner = {};
	$effect(() => {
		const sub = activeSub;
		if (!sub) {
			tabHeader.release(headerOwner);
			return;
		}
		tabHeader.claim(headerOwner, {
			key: 'subscriptions',
			title: sub.name,
			actions: headerActions,
			replace: editingId === sub.id ? headerRename : undefined,
			body: expanded.has(sub.id) ? headerBody : undefined,
			popover: menu === sub.id ? headerMenu : undefined,
		});
	});
	onDestroy(() => tabHeader.release(headerOwner));

	let menuTop = $state(68);
	let viewEl = $state<HTMLElement | null>(null);

	let activeMenuSub = $derived(
		$catalogStore.subscriptions.find((s) => s.id === menu),
	);
	async function activateSubscription(id: string) {
		try {
			await catalog.selectSubscription(id);
		} catch (cause) {
			error = t('errors.' + errorCode(cause));
		}
	}

	function selectSubscription(id: string) {
		if (activeSubId !== id) void activateSubscription(id);
	}

	function toggleExpanded(id: string) {
		if (expanded.has(id)) expanded.delete(id);
		else expanded.add(id);
		const known = new Set($catalogStore.subscriptions.map((sub) => sub.id));
		saveExpandedIds([...expanded].filter((key) => known.has(key)));
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
		if (busy) return;
		busy = id;
		error = '';
		try {
			await refreshSubscriptionWithStatus(id);
		} catch (e) {
			error = t('errors.' + errorCode(e));
		} finally {
			busy = null;
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

{#snippet headerRename()}
	{#if activeSub}
		<SubscriptionRenameField
			bind:name={editName}
			bind:inputEl={editInputEl}
			onSave={() => saveEditing(activeSub.id)}
			onCancel={cancelEditing}
		/>
	{/if}
{/snippet}

<!-- The active subscription lives in the shared header; these fill it. -->
{#snippet headerActions()}
	{#if activeSub}
		<SubscriptionCardActions
			name={activeSub.name}
			busy={busy !== null}
			menuOpen={menu === activeSub.id}
			expanded={expanded.has(activeSub.id)}
			onRefresh={() => refresh(activeSub.id)}
			onToggleMenu={(e) => toggleMenu(activeSub.id, e)}
			onToggleExpanded={() => toggleExpanded(activeSub.id)}
		/>
	{/if}
{/snippet}

{#snippet headerBody()}
	{#if activeSub}
		<SubscriptionDetails sub={activeSub} />
	{/if}
{/snippet}

{#snippet headerMenu()}
	<PopupMenu top="calc(var(--design-header-height) + 8px)" right="20px">
		{#if activeMenuSub}
			<SubscriptionMenuItems
				sub={activeMenuSub}
				deleteDisabled={deleteBlocked}
				onEdit={startEditing}
				onShare={showSubscriptionSource}
				onDelete={requestDelete}
			/>
		{/if}
	</PopupMenu>
{/snippet}

<section
	class="subscriptions-view"
	bind:this={viewEl}
	aria-label={t('subscriptions.title')}
>
	{#if error}<p role="alert" transition:rise>{error}</p>{/if}
	<div class="cards-list" onscroll={() => (menu = null)}>
		{#each otherSubs as sub (sub.id)}
			<div
				class="sub-card-wrapper"
				class:has-open-menu={menu === sub.id}
				animate:flip={{ duration: 280, easing: easeOut }}
				in:pop={{ start: 0.96, duration: 220 }}
				out:pop={{ start: 0.96, duration: 180 }}
			>
				<article class="sub-card" class:expanded={expanded.has(sub.id)}>
					<div class="card-header" class:is-editing={editingId === sub.id}>
						{#if editingId === sub.id}
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
								aria-pressed={false}
								onclick={() => selectSubscription(sub.id)}
							>
								<span class="sub-name">{sub.name}</span>
							</button>
							<div class="actions-group">
								<SubscriptionCardActions
									name={sub.name}
									busy={busy !== null}
									menuOpen={menu === sub.id}
									expanded={expanded.has(sub.id)}
									onRefresh={() => refresh(sub.id)}
									onToggleMenu={(e) => toggleMenu(sub.id, e)}
									onToggleExpanded={() => toggleExpanded(sub.id)}
								/>
							</div>
						{/if}
					</div>
					<div
						class="card-body"
						class:expanded={expanded.has(sub.id)}
						role="presentation"
						onclick={() => selectSubscription(sub.id)}
					>
						{#if expanded.has(sub.id)}
							<div transition:reveal>
								<SubscriptionDetails {sub} />
							</div>
						{/if}
					</div>
				</article>
			</div>
		{/each}
		{#if !$catalogStore.subscriptions.length}
			<p>{t('subscriptions.empty')}</p>
		{/if}
	</div>

	{#if menu && activeMenuSub && menu !== activeSub?.id}
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
		description={t('subscriptions.deleteConfirm', { name: deleting.name })}
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
		gap: 6px;
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
	}
	.sub-card-wrapper.has-open-menu {
		z-index: 50;
	}
	.sub-card {
		background: var(--design-panel);
		border: none;
		border-radius: 33px;
		padding: 19px 20px 19px 23px;
		display: flex;
		flex-direction: column;
		min-height: 66px;
		transition:
			background-color 200ms ease,
			box-shadow 200ms ease,
			transform 220ms var(--ease-spring);
		cursor: pointer;
	}
	.sub-card:hover {
		background: var(--design-panel-open);
		border: none;
	}
	.sub-card.expanded {
		background: var(--design-panel-open);
		border: none;
		min-height: 117px;
		box-shadow:
			var(--design-edge),
			inset 0 0 21.7px rgba(255, 255, 255, 0.02);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}
	.card-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		height: 28px;
		gap: 8px;
	}
	/* Icons keep their built-in padding, as in the Figma frame. */
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
	.sub-card:not(.expanded) .sub-name {
		opacity: 0.5;
	}
	.actions-group {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.card-body {
		margin-top: 0;
		color: inherit;
		width: 100%;
		text-align: left;
		cursor: pointer;
		display: flex;
		flex-direction: column;
		transition: margin-top 220ms ease;
	}
	.card-body.expanded {
		margin-top: 20px;
	}
</style>
