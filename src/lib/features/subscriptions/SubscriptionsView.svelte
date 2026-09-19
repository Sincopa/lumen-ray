<script lang="ts">
	import { onDestroy } from 'svelte';
	import { createDeferredTask } from '../../utils/deferredTask';
	import { slide } from 'svelte/transition';
	import { catalog, connection } from '../../services';
	import { errorCode } from '../../services/errors';
	import { t, i18n } from '../../i18n';
	import type { Subscription } from '../../services/catalog';
	import Dialog from '../../components/Dialog.svelte';
	import StatusDot from '../../components/StatusDot.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import PopupMenu from '../../components/PopupMenu.svelte';
	import MenuItem from '../../components/MenuItem.svelte';
	import reloadIcon from '../../../assets/reload.svg';
	import menuIcon from '../../../assets/menu.svg';
	import pencilIcon from '../../../assets/pencil.svg';
	import trashIcon from '../../../assets/trash.svg';
	import shareIcon from '../../../assets/share.svg';
	import chevronIcon from '../../../assets/chevron.svg';
	import checkIcon from '../../../assets/check.svg';
	import crossIcon from '../../../assets/cross.svg';
	import { formatSubscriptionUpdatedAgo } from '../../utils/time';

	const focusTask = createDeferredTask();
	onDestroy(focusTask.cancel);

	const catalogStore = catalog.state;
	const connectionStore = connection.state;

	// Local state
	let expanded = $state<string | null>(null);
	let editingId = $state<string | null>(null);
	let editName = $state('');
	let editInputEl = $state<HTMLInputElement | null>(null);
	let deleting = $state<Subscription | null>(null);
	let menu = $state<string | null>(null);
	let busy = $state<string | null>(null);
	let error = $state('');
	let exported = $state('');

	let activeSubId = $derived(
		$catalogStore.selectedSubscriptionId ??
			$catalogStore.servers.find((s) => s.id === $catalogStore.selectedServerId)
				?.subscriptionId ??
			$catalogStore.subscriptions[0]?.id,
	);

	let menuTop = $state(68);
	let viewEl = $state<HTMLElement | null>(null);

	let activeMenuSub = $derived(
		$catalogStore.subscriptions.find((s) => s.id === menu),
	);

	function formatUpdatedAgo(sub: Subscription): string {
		return formatSubscriptionUpdatedAgo(
			sub.updatedAt,
			sub.updatedMinutesAgo,
			i18n.currentLanguage,
		);
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
			await catalog.refreshSubscription(id);
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

	function saveEditing(subId: string) {
		focusTask.cancel();
		const trimmed = editName.trim();
		if (!trimmed) return;
		try {
			catalog.renameSubscription(subId, trimmed);
			editingId = null;
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	function remove() {
		if (!deleting) return;
		try {
			catalog.removeSubscription(deleting.id);
			deleting = null;
			error = '';
		} catch (e) {
			error = t('errors.' + errorCode(e));
		}
	}

	function exportSub(sub: Subscription) {
		exported = JSON.stringify(
			{
				version: 1,
				demo: true,
				name: sub.name,
				servers: $catalogStore.servers.filter(
					(s) => s.subscriptionId === sub.id,
				),
			},
			null,
			2,
		);
		menu = null;
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

<section
	class="subscriptions-view"
	bind:this={viewEl}
	aria-label={t('subscriptions.title')}
>
	{#if error}<p role="alert">{error}</p>{/if}
	<div class="cards-list" onscroll={() => (menu = null)}>
		{#each $catalogStore.subscriptions as sub (sub.id)}
			<div class="sub-card-wrapper" class:has-open-menu={menu === sub.id}>
				<article class="sub-card" class:expanded={expanded === sub.id}>
					<div class="card-header" class:is-editing={editingId === sub.id}>
						{#if editingId === sub.id}
							<div
								class="card-rename-inline"
								role="presentation"
								onclick={(e) => e.stopPropagation()}
								onkeydown={(e) => e.stopPropagation()}
							>
								<img src={pencilIcon} alt="" class="rename-icon" />
								<input
									bind:this={editInputEl}
									type="text"
									class="rename-input"
									bind:value={editName}
									maxlength="80"
									autocomplete="off"
									spellcheck="false"
									onkeydown={(e) => {
										if (e.key === 'Enter') {
											e.preventDefault();
											saveEditing(sub.id);
										} else if (e.key === 'Escape') {
											e.preventDefault();
											cancelEditing();
										}
									}}
								/>
								<button
									type="button"
									class="rename-action-btn check"
									onclick={() => saveEditing(sub.id)}
									aria-label={t('common.save')}
								>
									<img src={checkIcon} alt="" class="action-svg" />
								</button>
								<button
									type="button"
									class="rename-action-btn cancel"
									onclick={cancelEditing}
									aria-label={t('common.close')}
								>
									<img src={crossIcon} alt="" class="action-svg" />
								</button>
							</div>
						{:else}
							<button
								class="title-group"
								aria-expanded={expanded === sub.id}
								onclick={() => {
									catalog.selectSubscription(sub.id);
									expanded = expanded === sub.id ? null : sub.id;
								}}
							>
								<span class="sub-name">{sub.name}</span>
								{#if activeSubId === sub.id}
									<StatusDot variant="active" />
								{/if}
							</button>
							<div class="actions-group">
								<IconButton
									icon={reloadIcon}
									ariaLabel={t('common.reload')}
									disabled={busy !== null}
									onclick={() => refresh(sub.id)}
								/>
								<IconButton
									icon={menuIcon}
									ariaLabel={t('common.menu')}
									active={menu === sub.id}
									expanded={menu === sub.id}
									onclick={(e) => toggleMenu(sub.id, e)}
								/>
								<IconButton
									icon={chevronIcon}
									ariaLabel={sub.name}
									expanded={expanded === sub.id}
									class="card-chevron"
									onclick={() => {
										catalog.selectSubscription(sub.id);
										expanded = expanded === sub.id ? null : sub.id;
									}}
								/>
							</div>
						{/if}
					</div>
					{#if expanded === sub.id}
						<div class="card-body" transition:slide={{ duration: 240 }}>
							<div class="metrics-row">
								<div class="metric-item">
									<span class="metric-label">
										{t('subscriptions.daysLabel')}
									</span>
									<span class="metric-val">{sub.daysRemaining ?? '—'}</span>
								</div>
								<div class="metric-item">
									<span class="metric-label">
										{t('subscriptions.trafficLabel')}
									</span>
									<span class="metric-val">{sub.traffic ?? '—'}</span>
								</div>
							</div>
							<div class="updated-time">
								{formatUpdatedAgo(sub)}
							</div>
						</div>
					{/if}
				</article>
			</div>
		{:else}<p>{t('subscriptions.empty')}</p>{/each}
	</div>

	{#if menu && activeMenuSub}
		<PopupMenu top="{menuTop}px" right="20px">
			<MenuItem
				icon={pencilIcon}
				label={t('common.edit')}
				onclick={() => {
					if (activeMenuSub) startEditing(activeMenuSub);
				}}
			/>
			<MenuItem
				icon={shareIcon}
				label={t('common.export')}
				onclick={() => {
					if (activeMenuSub) exportSub(activeMenuSub);
				}}
			/>
			<MenuItem
				icon={trashIcon}
				label={t('common.delete')}
				variant="danger"
				disabled={$connectionStore.busy ||
					$connectionStore.status === 'connected'}
				onclick={() => {
					deleting = activeMenuSub;
					menu = null;
				}}
			/>
		</PopupMenu>
	{/if}
</section>

{#if deleting}
	<Dialog title={t('common.delete')} onClose={() => (deleting = null)}>
		<p>{t('subscriptions.deleteConfirm', { name: deleting.name })}</p>
		{#if error}
			<p role="alert">{error}</p>
		{/if}
		<button class="danger" onclick={remove}>{t('common.delete')}</button>
	</Dialog>
{/if}

{#if exported}
	<Dialog title={t('common.export')} onClose={() => (exported = '')}>
		<p>{t('demo.export')}</p>
		<textarea
			readonly
			value={exported}
			aria-label={t('common.export')}
			rows="10"></textarea>
	</Dialog>
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
		gap: 10px;
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
		background: rgba(0, 0, 0, 0.22);
		border: none;
		border-radius: 32px;
		padding: 18px 22px;
		display: flex;
		flex-direction: column;
	}
	.sub-card:hover,
	.sub-card.expanded {
		background: rgba(0, 0, 0, 0.3);
		border: none;
	}
	.card-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		height: 24px;
		gap: 8px;
	}
	.card-rename-inline {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		height: 100%;
	}
	.rename-icon {
		width: 16px;
		height: 16px;
		filter: brightness(0) invert(1);
		opacity: 0.6;
		flex-shrink: 0;
	}
	.rename-input {
		flex: 1;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		font-size: 14px;
		font-weight: 500;
		color: #ffffff !important;
		padding: 0 !important;
		letter-spacing: -0.2px;
	}
	.rename-action-btn {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: transparent;
		border: none;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		flex-shrink: 0;
		transition:
			background 150ms ease,
			transform 150ms ease;
	}
	.rename-action-btn:hover {
		background: rgba(255, 255, 255, 0.12);
		transform: scale(1.08);
	}
	.rename-action-btn.check:hover {
		background: rgba(134, 225, 111, 0.2);
	}
	.rename-action-btn.cancel:hover {
		background: rgba(243, 57, 57, 0.2);
	}
	.action-svg {
		width: 14px;
		height: 14px;
		filter: brightness(0) invert(1);
		opacity: 0.75;
	}
	.rename-action-btn:hover .action-svg {
		opacity: 1;
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
	.sub-name {
		font-size: 15px;
		font-weight: 500;
		letter-spacing: -0.2px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.actions-group {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	:global(.card-chevron) {
		transition: transform 0.28s;
	}
	:global(.card-chevron.expanded) {
		transform: rotate(180deg);
	}
	.card-body {
		margin-top: 12px;
		padding-top: 11px;
		border-top: none;
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
		gap: 12px;
	}
	.metrics-row {
		display: flex;
		gap: 24px;
	}
	.metric-item {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.metric-label {
		font-size: 10px;
		color: #ffffff80;
	}
	.metric-val {
		font-size: 13px;
		font-weight: 600;
	}
	.updated-time {
		font-size: 10px;
		color: #ffffff8c;
		max-width: 50%;
		text-align: right;
	}
	@media (prefers-reduced-motion: reduce) {
		:global(.card-chevron) {
			transition: none;
		}
	}
</style>
