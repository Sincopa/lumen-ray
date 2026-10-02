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

	type Tab = 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules';
	interface Props {
		currentTab: Tab;
		routingMode?: 'global' | 'rules';
		hasSubscriptions?: boolean;
		modeDisabled?: boolean;
		onTabChange: (tab: Tab) => void;
		onModeChange?: (mode: 'global' | 'rules') => void;
		onActionClick?: () => void;
	}
	let {
		currentTab,
		routingMode = 'global',
		hasSubscriptions = true,
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

<nav class="bottom-nav" aria-label={t('nav.home')}>
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
	{#if currentTab === 'subscriptions'}
		<button
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
			type="button"
			class="action-pill"
			disabled={modeDisabled}
			aria-label={modeLabel}
			aria-pressed={routingMode === 'rules'}
			use:tooltip={modeLabel}
			onclick={() =>
				onModeChange?.(routingMode === 'global' ? 'rules' : 'global')}
		>
			<img
				src={routingMode === 'rules' ? tunIcon : globalIcon}
				alt=""
				width="34"
				height="34"
			/>
			<span>{routingMode === 'rules' ? 'Tun' : 'All'}</span>
		</button>
	{/if}
</nav>

<style>
	.bottom-nav {
		position: absolute;
		bottom: var(--nav-bottom);
		left: var(--nav-inset);
		right: var(--nav-inset);
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
		width: 50px;
		background: var(--design-glass);
		backdrop-filter: blur(25px);
	}
	.action-pill {
		min-width: 87px;
		padding: 8px 18px 8px 8px;
		background: #fff;
		color: #252525;
		font-size: 16px;
		font-weight: 500;
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
