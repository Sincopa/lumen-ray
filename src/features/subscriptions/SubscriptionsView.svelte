<script lang="ts">
	import closeBoldIcon from '../../assets/close-bold.svg';
	import copyIcon from '../../assets/copy.svg';
	import { onDestroy } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { createDeferredTask } from '../../lib/utils/deferredTask';
	import { flip } from 'svelte/animate';
	import { reveal, rise, pop, swap, easeOut } from '../../lib/motion';
	import { catalog, connection } from '../../lib/services';
	import { errorCode } from '../../lib/services/errors';
	import {
		refreshStatus,
		refreshSubscriptionWithStatus,
	} from '../../lib/services/maintenance';
	import { t, i18n } from '../../lib/i18n';
	import type { Subscription } from '../../lib/services/catalog';
	import DesignOverlay from '../../components/DesignOverlay.svelte';
	import ConfirmDialog from '../../components/ConfirmDialog.svelte';
	import StatusDot from '../../components/StatusDot.svelte';
	import IconButton from '../../components/IconButton.svelte';
	import PopupMenu from '../../components/PopupMenu.svelte';
	import MenuItem from '../../components/MenuItem.svelte';
	import MaterialIcon from '../../components/MaterialIcon.svelte';
	import reloadIcon from '../../assets/reload.svg';
	import menuIcon from '../../assets/menu.svg';
	import pencilIcon from '../../assets/pencil.svg';
	import trashIcon from '../../assets/trash.svg';
	import shareIcon from '../../assets/share-link.svg';
	import chevronIcon from '../../assets/chevron-up.svg';
	import backIcon from '../../assets/back.svg';
	import checkIcon from '../../assets/check.svg';
	import { formatSubscriptionUpdatedAgo } from '../../lib/utils/time';

	const focusTask = createDeferredTask();
	let copyResetTimer: ReturnType<typeof setTimeout> | undefined;
	onDestroy(() => {
		focusTask.cancel();
		if (copyResetTimer) clearTimeout(copyResetTimer);
	});

	const catalogStore = catalog.state;
	const connectionStore = connection.state;
	const subscriptionRefreshStatus = refreshStatus;

	// Expanded cards survive restarts; only the chevron changes them.
	const EXPANDED_KEY = 'lumen_expanded_subscriptions';
	function loadExpanded(): string[] {
		try {
			const ids: unknown = JSON.parse(
				localStorage.getItem(EXPANDED_KEY) ?? '[]',
			);
			return Array.isArray(ids)
				? ids.filter((id): id is string => typeof id === 'string')
				: [];
		} catch {
			return [];
		}
	}

	// Local state
	const expanded = new SvelteSet<string>(loadExpanded());
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
	let copyState = $state<'idle' | 'copied' | 'failed'>('idle');

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
		try {
			localStorage.setItem(
				EXPANDED_KEY,
				JSON.stringify([...expanded].filter((key) => known.has(key))),
			);
		} catch {
			/* Storage is optional. */
		}
	}

	function formatUpdatedAgo(sub: Subscription): string {
		const minutes =
			sub.updatedMinutesAgo ??
			Math.max(
				1,
				Math.floor(
					(Date.now() - (sub.updatedAt || Date.now() - 60_000)) / 60_000,
				),
			);
		if (minutes < 60)
			return t('subscriptions.updatedMinutesAgo', { count: minutes });
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
		copyState = 'idle';
		try {
			sharedSource = await catalog.getSubscriptionSource(sub.id);
		} catch (cause) {
			shareError = t('subscriptions.sourceUnavailable');
			console.error('[subscription-source] Unable to retrieve source', cause);
		}
	}

	async function copySubscriptionSource() {
		if (!sharedSource) return;
		copyState = 'idle';
		try {
			if (navigator.clipboard?.writeText) {
				await navigator.clipboard.writeText(sharedSource);
			} else {
				const textarea = document.createElement('textarea');
				textarea.value = sharedSource;
				textarea.style.position = 'fixed';
				textarea.style.opacity = '0';
				document.body.appendChild(textarea);
				textarea.select();
				const copied = document.execCommand('copy');
				textarea.remove();
				if (!copied) throw new Error('Clipboard unavailable');
			}
			copyState = 'copied';
			if (copyResetTimer) clearTimeout(copyResetTimer);
			copyResetTimer = setTimeout(() => {
				copyState = 'idle';
			}, 1800);
		} catch {
			copyState = 'failed';
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

<section
	class="subscriptions-view"
	bind:this={viewEl}
	aria-label={t('subscriptions.title')}
>
	{#if error}<p role="alert" transition:rise>{error}</p>{/if}
	<div class="cards-list" onscroll={() => (menu = null)}>
		{#each $catalogStore.subscriptions as sub (sub.id)}
			<div
				class="sub-card-wrapper"
				class:has-open-menu={menu === sub.id}
				animate:flip={{ duration: 280, easing: easeOut }}
				out:pop={{ start: 0.96, duration: 180 }}
			>
				<article class="sub-card" class:expanded={expanded.has(sub.id)}>
					<div class="card-header" class:is-editing={editingId === sub.id}>
						{#if editingId === sub.id}
							<div
								in:swap
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
									<img src={closeBoldIcon} alt="" class="action-svg" />
								</button>
							</div>
						{:else}
							<button
								in:swap
								class="title-group"
								aria-pressed={activeSubId === sub.id}
								onclick={() => selectSubscription(sub.id)}
							>
								<span class="sub-name">{sub.name}</span>
								{#if activeSubId === sub.id}
									<span class="dot-pop" in:pop={{ start: 0.6 }}>
										<StatusDot variant="active" />
									</span>
								{/if}
							</button>
							<div class="actions-group">
								<IconButton
									design
									icon={reloadIcon}
									ariaLabel={t('common.reload')}
									disabled={busy !== null}
									onclick={() => refresh(sub.id)}
								/>
								<IconButton
									design
									icon={menuIcon}
									ariaLabel={t('common.menu')}
									active={menu === sub.id}
									expanded={menu === sub.id}
									onclick={(e) => toggleMenu(sub.id, e)}
								/>
								<IconButton
									design
									icon={chevronIcon}
									ariaLabel={sub.name}
									expanded={expanded.has(sub.id)}
									class="card-chevron"
									onclick={() => toggleExpanded(sub.id)}
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
							<div class="card-details" transition:reveal>
								<div class="metrics-row">
									<div class="metric-item">
										<span class="metric-label">
											{t('subscriptions.daysLabel')}
										</span>
										<span class="metric-val">
											{sub.daysRemaining ?? (sub.traffic === '∞' ? '∞' : '—')}
										</span>
									</div>
									<div class="metric-item">
										<span class="metric-label">
											{t('subscriptions.trafficLabel')}
										</span>
										<span class="metric-val">{sub.traffic ?? '—'}</span>
									</div>
								</div>
								<div class="updated-time">{formatUpdatedAgo(sub)}</div>
								{#if $subscriptionRefreshStatus[sub.id]?.state === 'error'}
									<div class="refresh-error" role="status">
										{t('subscriptions.refreshFailed')}: {t(
											'errors.' +
												($subscriptionRefreshStatus[sub.id].error ?? 'unknown'),
										)}
									</div>
								{/if}
							</div>
						{/if}
					</div>
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
				label={t('common.share')}
				onclick={() => {
					if (activeMenuSub) void showSubscriptionSource(activeMenuSub);
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
	<DesignOverlay
		labelledby="share-title"
		describedby="share-hint"
		onDismiss={() => (sharing = null)}
	>
		<div class="share-content">
			<div class="share-copy">
				<h2 id="share-title">
					{t('subscriptions.shareTitle', { name: sharing.name })}
				</h2>
				<p id="share-hint" class="share-hint">
					{t('subscriptions.shareHint')}
				</p>
			</div>
			{#if sharedSource}
				<div class="source-field">
					<input
						readonly
						value={sharedSource}
						aria-label={t('subscriptions.source')}
						onfocus={(event) => event.currentTarget.select()}
					/>
					<button
						class="copy-icon-button"
						class:copied={copyState === 'copied'}
						type="button"
						onclick={copySubscriptionSource}
						aria-label={copyState === 'copied'
							? t('common.copied')
							: t('common.copy')}
						title={copyState === 'copied'
							? t('common.copied')
							: t('common.copy')}
					>
						<span class="copy-icon">
							{#if copyState === 'copied'}
								<MaterialIcon name="check" size={20} />
							{:else}
								<img src={copyIcon} alt="" width="34" height="34" />
							{/if}
						</span>
					</button>
				</div>
				{#if copyState === 'failed'}
					<p class="copy-error" role="alert">{t('subscriptions.copyFailed')}</p>
				{/if}
			{:else if shareError}
				<p class="share-error" role="alert">{shareError}</p>
			{:else}
				<p class="share-loading">{t('common.loading')}</p>
			{/if}
		</div>
		{#snippet footer()}
			<IconButton
				design
				icon={backIcon}
				ariaLabel={t('common.back')}
				size="lg"
				variant="surface"
				onclick={() => (sharing = null)}
			/>
		{/snippet}
	</DesignOverlay>
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
	.dot-pop {
		display: inline-flex;
		flex-shrink: 0;
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
		border-radius: 32px;
		padding: 20px 20px 20px 23px;
		display: flex;
		flex-direction: column;
		min-height: 68px;
		transition:
			background-color 200ms ease,
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
	.card-rename-inline {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		height: 100%;
		margin-left: -7px;
	}
	.rename-icon {
		width: 28px;
		height: 28px;
		flex-shrink: 0;
	}
	.rename-input {
		flex: 1;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		font-size: 16px;
		font-weight: 500;
		color: #ffffff !important;
		padding: 0 !important;
		letter-spacing: -0.2px;
	}
	.rename-action-btn {
		width: 28px;
		height: 28px;
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
	.rename-action-btn.check {
		width: 34px;
		height: 34px;
		margin: -3px 0;
	}
	.action-svg {
		display: block;
		width: 100%;
		height: 100%;
		opacity: 0.85;
	}
	.rename-action-btn.check .action-svg {
		filter: brightness(0) invert(1);
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
	/* Not cap-trimmed: the ellipsis clip would cut ascenders and descenders. */
	.sub-name {
		font-size: 16px;
		font-weight: 500;
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
	:global(.card-chevron) {
		transform: rotate(180deg);
		transition: transform 200ms var(--ease-out);
	}
	:global(.card-chevron.expanded) {
		transform: rotate(0);
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
	.card-details {
		display: flex;
		width: 100%;
		justify-content: space-between;
		align-items: flex-end;
		gap: 12px;
	}
	.metrics-row {
		display: flex;
		gap: 24px;
	}
	.metric-item:first-child {
		min-width: 43px;
	}
	.metric-item {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.metric-label {
		font-size: 12px;
		line-height: 12px;
		font-weight: 500;
		color: #ffffff40;
	}
	.metric-val {
		font-size: 12px;
		line-height: 12px;
		font-weight: 500;
		letter-spacing: -0.24px;
	}
	.updated-time {
		font-size: 12px;
		line-height: 14px;
		color: #ffffff40;
		max-width: 50%;
		text-align: right;
	}
	.refresh-error {
		font-size: 11px;
		color: #ffb4aa;
		text-align: right;
	}
	.share-content {
		display: flex;
		flex-direction: column;
		gap: 48px;
		text-align: center;
	}
	.share-copy {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.share-copy h2 {
		margin: 0;
		font-size: 28px;
		font-weight: 500;
		line-height: 28px;
	}
	.share-hint,
	.share-loading {
		margin: 0;
		color: #ffffff80;
		font-size: 14px;
		line-height: 18.2px;
		letter-spacing: -0.3px;
		text-align: center;
	}
	.source-field {
		display: flex;
		align-items: center;
		gap: 24px;
		width: 100%;
		height: 64px;
		padding: 7px 7px 7px 32px;
		border-radius: 123px;
		background: #ffffff0d;
		box-sizing: border-box;
	}
	.source-field input {
		flex: 1;
		min-width: 0;
		padding: 0;
		border: 0;
		background: transparent;
		color: #fff;
		font-size: 18px;
		line-height: 18px;
		letter-spacing: -0.4px;
		text-overflow: ellipsis;
		outline: none;
	}
	.copy-icon-button {
		display: grid;
		place-items: center;
		flex: 0 0 50px;
		width: 50px;
		height: 50px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: #ffffff0d;
		color: #fff;
		transition:
			transform 180ms var(--ease-spring),
			background-color 220ms ease,
			color 220ms ease;
	}
	.copy-icon-button:hover {
		background: #ffffff1a;
	}
	.copy-icon-button:active {
		transform: scale(0.92);
	}
	.copy-icon-button.copied {
		background: var(--color-success);
		color: #10210d;
	}
	.copy-icon {
		display: grid;
		place-items: center;
	}
	.copy-icon-button.copied .copy-icon {
		animation: copy-confirm 420ms var(--ease-spring);
	}
	@keyframes copy-confirm {
		0% {
			transform: scale(0.55) rotate(-18deg);
			opacity: 0;
		}
		55% {
			transform: scale(1.18) rotate(3deg);
			opacity: 1;
		}
		100% {
			transform: scale(1) rotate(0);
			opacity: 1;
		}
	}
	.copy-error,
	.share-error {
		margin: 0;
		border-radius: 14px;
		background: rgba(243, 57, 57, 0.12);
		padding: 10px 12px;
		font-size: 12px;
		line-height: 1.4;
	}
	@media (prefers-reduced-motion: reduce) {
		:global(.card-chevron) {
			transition: none;
		}
		.copy-icon-button.copied .copy-icon {
			animation: none;
		}
	}
</style>
