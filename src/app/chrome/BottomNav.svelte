<script lang="ts">
	import logoIcon from '../../assets/logo.svg';
	import subscriptionsIcon from '../../assets/subscriptions.svg';
	import serversIcon from '../../assets/rules.svg';
	import rulesSettingsIcon from '../../assets/rules-settings-v2.svg';
	import plusIcon from '../../assets/plus.svg';
	import globalIcon from '../../assets/mode-global.svg';
	import tunIcon from '../../assets/mode-rules.svg';
	import trashIcon from '../../assets/trash.svg';
	import checkIcon from '../../assets/check.svg';
	import { tooltip } from '../../lib/actions/tooltip';
	import { t } from '../../lib/i18n';
	import { fade } from 'svelte/transition';
	import { pop, rise } from '../../lib/motion';
	import Morph from '../../components/Morph.svelte';
	import type { SelectionBar } from '../shell.svelte';

	type Tab = 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules';
	interface Props {
		currentTab: Tab;
		routingMode?: 'global' | 'rules';
		hasSubscriptions?: boolean;
		/** Replaces the routing-mode pill with the "add profile" button. */
		showAdd?: boolean;
		modeDisabled?: boolean;
		/** Rule selection turns the tabs into "delete" and the action into "done". */
		selection?: SelectionBar | null;
		onTabChange: (tab: Tab) => void;
		onModeChange?: (mode: 'global' | 'rules') => void;
		onActionClick?: () => void;
	}
	let {
		currentTab,
		routingMode = 'global',
		hasSubscriptions = true,
		showAdd = false,
		modeDisabled = false,
		selection = null,
		onTabChange,
		onModeChange,
		onActionClick,
	}: Props = $props();
	const tabs = [
		{ id: 'home' as const, icon: logoIcon, label: 'nav.home' },
		{
			id: 'subscriptions' as const,
			icon: subscriptionsIcon,
			label: 'nav.subscriptions',
		},
		{ id: 'servers' as const, icon: serversIcon, label: 'nav.servers' },
		{ id: 'rules' as const, icon: rulesSettingsIcon, label: 'nav.rules' },
	];
	// One button morphs between "add", the routing mode and "done".
	let action = $derived(
		selection
			? 'done'
			: showAdd
				? 'add'
				: hasSubscriptions
					? routingMode
					: null,
	);
	let actionLabel = $derived(
		action === 'done'
			? t('common.close')
			: action === 'add'
				? t('subscriptions.addProfile')
				: t(routingMode === 'rules' ? 'home.modeRules' : 'home.modeGlobal'),
	);
</script>

<nav
	in:rise|global={{ y: 16, duration: 260 }}
	out:fade|global={{ duration: 120 }}
	class="bottom-nav"
	class:on-dark={currentTab !== 'home' || !hasSubscriptions}
	aria-label={t('nav.home')}
>
	<div class="nav-pill" class:danger={!!selection}>
		<Morph key={selection ? 'select' : 'tabs'} anchor="start">
			{#if selection}
				<button
					type="button"
					class="delete-btn"
					aria-label={t('rules.deleteSelected')}
					use:tooltip={t('rules.deleteSelected')}
					disabled={!selection.count}
					onclick={() => selection?.onDelete()}
				>
					<img src={trashIcon} alt="" width="34" height="34" />
				</button>
			{:else}
				<span class="tab-row">
					{#each tabs as tab (tab.id)}
						<button
							type="button"
							class="pill-btn"
							class:active={currentTab === tab.id}
							aria-current={currentTab === tab.id ? 'page' : undefined}
							aria-label={t(tab.label)}
							use:tooltip={t(tab.label)}
							onclick={() => currentTab !== tab.id && onTabChange(tab.id)}
						>
							<img src={tab.icon} alt="" width="34" height="34" />
						</button>
					{/each}
				</span>
			{/if}
		</Morph>
	</div>
	{#if action}
		<button
			in:pop={{ start: 0.9 }}
			type="button"
			class="nav-action"
			class:light={action !== 'add'}
			disabled={(action === 'global' || action === 'rules') && modeDisabled}
			aria-label={actionLabel}
			aria-pressed={action === 'global' || action === 'rules'
				? routingMode === 'rules'
				: undefined}
			use:tooltip={actionLabel}
			onclick={() => {
				if (action === 'done') selection?.onDone();
				else if (action === 'add') onActionClick?.();
				else onModeChange?.(routingMode === 'global' ? 'rules' : 'global');
			}}
		>
			<Morph key={action}>
				{#if action === 'done'}
					<span class="glyph">
						<img src={checkIcon} alt="" width="34" height="34" />
					</span>
				{:else if action === 'add'}
					<span class="glyph">
						<img src={plusIcon} alt="" width="21" height="21" />
					</span>
				{:else}
					<span class="mode">
						<img
							src={action === 'rules' ? tunIcon : globalIcon}
							alt=""
							width="34"
							height="34"
						/>
						<span>{action === 'rules' ? 'Tun' : 'All'}</span>
					</span>
				{/if}
			</Morph>
		</button>
	{/if}
</nav>

<style>
	.bottom-nav {
		position: absolute;
		bottom: var(--nav-bottom);
		left: var(--nav-inset);
		right: var(--nav-inset-end);
		height: var(--nav-height);
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		z-index: 20;
	}
	.nav-pill {
		display: flex;
		align-items: center;
		height: 50px;
		padding: 8px;
		box-sizing: border-box;
		border-radius: 123px;
		background: var(--design-glass);
		backdrop-filter: blur(25px);
		transition: background-color 300ms ease;
	}
	/* The tab pill shrinks into the red delete button. */
	.nav-pill.danger {
		background: #d03441;
	}
	.tab-row {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.pill-btn {
		display: grid;
		place-items: center;
		width: 34px;
		height: 34px;
		flex: 0 0 34px;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		transition:
			background-color 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.pill-btn img {
		display: block;
		opacity: 0.5;
		transition: opacity 160ms ease;
	}
	.pill-btn.active img {
		opacity: 1;
	}
	/* Fills the whole red circle, so the click target matches what shows. */
	.delete-btn {
		display: grid;
		place-items: center;
		width: 50px;
		height: 50px;
		margin: -8px;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		transition:
			opacity 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.delete-btn img {
		filter: brightness(0) invert(1);
	}
	.delete-btn:disabled {
		opacity: 0.5;
	}
	.nav-action {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		height: var(--nav-height);
		padding: 8px;
		box-sizing: border-box;
		border: 1px solid transparent;
		border-radius: 123px;
		background: var(--design-glass);
		color: #262626;
		backdrop-filter: blur(25px);
		transition:
			background-color 300ms ease,
			border-color 300ms ease,
			transform 160ms var(--ease-spring);
	}
	/* Mode and "done" are white; "add" stays glass. */
	.nav-action.light {
		border-color: #ffffff40;
		background: #fff;
	}
	.on-dark .nav-pill:not(.danger),
	.on-dark .nav-action:not(.light) {
		background: var(--design-glass-dark);
	}
	.glyph {
		display: grid;
		place-items: center;
		width: 40px;
		height: 40px;
	}
	.mode {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 69px;
		padding-right: 10px;
		box-sizing: border-box;
		font-size: 20px;
		font-weight: 600;
		line-height: 26px;
		white-space: nowrap;
	}
	.mode img {
		object-fit: contain;
		filter: brightness(0);
	}
	.pill-btn:active,
	.delete-btn:active:not(:disabled),
	.nav-action:active:not(:disabled) {
		transform: scale(0.94);
	}
	/* Narrow windows keep the mode toggle as an icon so the row still fits. */
	@media (max-width: 350px) {
		.mode {
			justify-content: center;
			min-width: 40px;
			padding-right: 0;
		}
		.mode span {
			display: none;
		}
	}
	@media (hover: hover) and (pointer: fine) {
		.pill-btn:hover {
			background: #ffffff12;
		}
		.nav-action:not(.light):hover {
			background: #ffffff12;
		}
		.nav-action.light:hover:not(:disabled) {
			background: #f1f1f1;
		}
		.nav-pill.danger:has(.delete-btn:hover:not(:disabled)) {
			background: #e03d4b;
		}
	}
</style>
