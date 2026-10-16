<script lang="ts">
	import logoIcon from '../../assets/logo.svg';
	import subscriptionsIcon from '../../assets/subscriptions.svg';
	import serversIcon from '../../assets/rules.svg';
	import rulesSettingsIcon from '../../assets/rules-settings-v2.svg';
	import plusIcon from '../../assets/plus.svg';
	import globalIcon from '../../assets/mode-global.svg';
	import tunIcon from '../../assets/mode-rules.svg';
	import { tooltip } from '../actions/tooltip';
	import { t } from '../i18n';
	import { fade } from 'svelte/transition';
	import { pop, rise, swap } from '../motion';

	type Tab = 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules';
	interface Props {
		currentTab: Tab;
		routingMode?: 'global' | 'rules';
		hasSubscriptions?: boolean;
		/** Replaces the routing-mode pill with the "add profile" button. */
		showAdd?: boolean;
		modeDisabled?: boolean;
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
	let modeLabel = $derived(
		t(routingMode === 'rules' ? 'home.modeRules' : 'home.modeGlobal'),
	);
</script>

<nav
	in:rise|global={{ y: 16, duration: 260 }}
	out:fade|global={{ duration: 120 }}
	class="bottom-nav"
	class:on-dark={currentTab !== 'home' || !hasSubscriptions}
	aria-label={t('nav.home')}
>
	<div class="nav-pill">
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
	</div>
	{#if showAdd}
		<button
			in:pop={{ start: 0.9 }}
			type="button"
			class="nav-add"
			aria-label={t('subscriptions.addProfile')}
			use:tooltip={t('subscriptions.addProfile')}
			onclick={() => onActionClick?.()}
		>
			<img src={plusIcon} alt="" width="21" height="21" />
		</button>
	{:else if hasSubscriptions}
		<button
			in:pop={{ start: 0.9 }}
			type="button"
			class="action-pill"
			disabled={modeDisabled}
			aria-label={modeLabel}
			aria-pressed={routingMode === 'rules'}
			use:tooltip={modeLabel}
			onclick={() =>
				onModeChange?.(routingMode === 'global' ? 'rules' : 'global')}
		>
			{#key routingMode}
				<img
					in:pop={{ start: 0.8, duration: 220 }}
					src={routingMode === 'rules' ? tunIcon : globalIcon}
					alt=""
					width="34"
					height="34"
				/>
				<span in:swap>{routingMode === 'rules' ? 'Tun' : 'All'}</span>
			{/key}
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
		gap: 12px;
		height: 50px;
		padding: 8px;
		border-radius: 123px;
		background: var(--design-glass);
		backdrop-filter: blur(25px);
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
	.nav-add,
	.action-pill {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		height: 50px;
		border: 0;
		border-radius: 123px;
		padding: 8px;
		transition:
			background-color 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.nav-add {
		width: var(--nav-height);
		height: var(--nav-height);
		background: var(--design-glass);
		backdrop-filter: blur(25px);
	}
	.on-dark .nav-pill,
	.on-dark .nav-add {
		background: var(--design-glass-dark);
	}
	.action-pill {
		min-width: 87px;
		height: var(--nav-height);
		gap: 8px;
		padding: 8px 18px 8px 8px;
		border: 1px solid #ffffff40;
		background: #fff;
		color: #262626;
		font-size: 20px;
		font-weight: 600;
		line-height: 26px;
		backdrop-filter: blur(25px);
	}
	.action-pill img {
		object-fit: contain;
		filter: brightness(0);
	}
	.pill-btn:active,
	.nav-add:active,
	.action-pill:active:not(:disabled) {
		transform: scale(0.94);
	}
	/* Narrow windows keep the mode toggle as an icon so the row still fits. */
	@media (max-width: 350px) {
		.action-pill {
			min-width: 0;
			width: var(--nav-height);
			padding: 0;
		}
		.action-pill span {
			display: none;
		}
	}
	@media (hover: hover) and (pointer: fine) {
		.pill-btn:hover {
			background: #ffffff12;
		}
		.nav-add:hover {
			background: #ffffff12;
		}
		.action-pill:hover:not(:disabled) {
			background: #f1f1f1;
		}
	}
</style>
