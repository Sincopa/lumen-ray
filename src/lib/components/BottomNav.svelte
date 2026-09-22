<script lang="ts">
	import logoIcon from '../../assets/logo.svg';
	import subscriptionsIcon from '../../assets/subscriptions.svg';
	import serversIcon from '../../assets/servers.svg';
	import addIcon from '../../assets/add.svg';
	import globalIcon from '../../assets/mode-global.svg';
	import rulesIcon from '../../assets/mode-rules.svg';
	import IconButton from './IconButton.svelte';
	import { tooltip } from '../actions/tooltip';
	import { t } from '../i18n';

	interface Props {
		currentTab: 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules';
		routingMode?: 'global' | 'rules';
		hasSubscriptions?: boolean;
		modeDisabled?: boolean;
		onTabChange: (
			tab: 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules',
		) => void;
		onModeChange?: (mode: 'global' | 'rules') => void;
		onActionClick?: () => void;
	}

	let {
		currentTab = 'home',
		routingMode = 'global',
		hasSubscriptions = true,
		modeDisabled = false,
		onTabChange,
		onModeChange,
		onActionClick,
	}: Props = $props();

	function handleTabChange(
		tab: 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules',
	) {
		if (tab === currentTab) return;
		onTabChange(tab);
	}

	function toggleMode() {
		if (modeDisabled) return;
		const nextMode = routingMode === 'global' ? 'rules' : 'global';
		onModeChange?.(nextMode);
	}
</script>

<nav class="bottom-nav">
	<div class="nav-pill">
		<!-- Slot 1: Home (Lumen Logo) -->
		<button
			class="pill-btn"
			aria-current={currentTab === 'home' ? 'page' : undefined}
			class:active={currentTab === 'home'}
			onclick={() => handleTabChange('home')}
			aria-label={t('nav.home')}
			use:tooltip={t('nav.home')}
		>
			<span class="btn-surface" class:active-surface={currentTab === 'home'}>
				<img
					src={logoIcon}
					alt="Lumen"
					class="tab-icon logo-icon"
					class:active-icon={currentTab === 'home'}
				/>
			</span>
		</button>

		<!-- Slot 2: Subscriptions (subscriptions.svg) -->
		<button
			class="pill-btn"
			aria-current={currentTab === 'subscriptions' ? 'page' : undefined}
			class:active={currentTab === 'subscriptions'}
			onclick={() => handleTabChange('subscriptions')}
			aria-label={t('nav.subscriptions')}
			use:tooltip={t('nav.subscriptions')}
		>
			<span
				class="btn-surface"
				class:active-surface={currentTab === 'subscriptions'}
			>
				<img
					src={subscriptionsIcon}
					alt=""
					class="tab-icon sub-icon"
					class:active-icon={currentTab === 'subscriptions'}
				/>
			</span>
		</button>

		<!-- Slot 3: Servers (servers.svg) -->
		<button
			class="pill-btn"
			aria-current={currentTab === 'servers' ? 'page' : undefined}
			class:active={currentTab === 'servers'}
			onclick={() => handleTabChange('servers')}
			aria-label={t('nav.servers')}
			use:tooltip={t('nav.servers')}
		>
			<span class="btn-surface" class:active-surface={currentTab === 'servers'}>
				<img
					src={serversIcon}
					alt=""
					class="tab-icon srv-icon"
					class:active-icon={currentTab === 'servers'}
				/>
			</span>
		</button>

		<!-- Slot 4: Rules (mode-rules.svg) -->
		<button
			class="pill-btn"
			aria-current={currentTab === 'rules' ? 'page' : undefined}
			class:active={currentTab === 'rules'}
			onclick={() => handleTabChange('rules')}
			aria-label={t('nav.rules')}
			use:tooltip={t('nav.rules')}
		>
			<span class="btn-surface" class:active-surface={currentTab === 'rules'}>
				<img
					src={rulesIcon}
					alt=""
					class="tab-icon rules-icon"
					class:active-icon={currentTab === 'rules'}
				/>
			</span>
		</button>
	</div>

	{#if currentTab === 'subscriptions'}
		<div class="right-slot">
			<IconButton
				icon={addIcon}
				ariaLabel={t('subscriptions.addProfile') || t('common.add')}
				size="lg"
				variant="surface"
				onclick={() => onActionClick?.()}
			/>
		</div>
	{:else if hasSubscriptions}
		<div class="right-slot">
			<!-- Mode switcher pill (global/rules) — click anywhere to toggle -->
			<div
				class="action-pill"
				class:disabled={modeDisabled}
				role="button"
				tabindex={modeDisabled ? -1 : 0}
				aria-disabled={modeDisabled}
				aria-label={t('home.modeGlobal') + ' / ' + t('home.modeRules')}
				onclick={toggleMode}
				onkeydown={(e) => {
					if (e.key === 'Enter' || e.key === ' ') {
						e.preventDefault();
						toggleMode();
					}
				}}
			>
				<button
					type="button"
					class="action-pill-btn"
					class:action-active={routingMode === 'global'}
					disabled={modeDisabled}
					aria-pressed={routingMode === 'global'}
					aria-label={t('home.modeGlobal')}
					use:tooltip={t('home.modeGlobal')}
					onclick={(e) => {
						e.stopPropagation();
						toggleMode();
					}}
				>
					<img
						src={globalIcon}
						alt=""
						class="mode-icon"
						class:mode-active={routingMode === 'global'}
					/>
				</button>
				<button
					type="button"
					class="action-pill-btn"
					class:action-active={routingMode === 'rules'}
					disabled={modeDisabled}
					aria-pressed={routingMode === 'rules'}
					aria-label={t('home.modeRules')}
					use:tooltip={t('home.modeRules')}
					onclick={(e) => {
						e.stopPropagation();
						toggleMode();
					}}
				>
					<img
						src={rulesIcon}
						alt=""
						class="mode-icon"
						class:mode-active={routingMode === 'rules'}
					/>
				</button>
			</div>
		</div>
	{/if}
</nav>

<style>
	.bottom-nav {
		position: absolute;
		bottom: 33px;
		left: 16px;
		right: 16px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: auto;
		max-width: 326px;
		gap: 8px;
		flex-shrink: 0;
		margin: 0 auto;
		padding-top: 0;
		z-index: 20;
		pointer-events: auto;
	}

	.nav-pill {
		width: 184px;
		height: 50px;
		border-radius: 124px;
		background: rgba(0, 0, 0, 0.28);
		box-shadow:
			0 8px 32px rgba(0, 0, 0, 0.45),
			0 2px 8px rgba(0, 0, 0, 0.3);
		-webkit-backdrop-filter: blur(16px);
		backdrop-filter: blur(16px);
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 5px 6px;
		box-sizing: border-box;
		border: none;
	}

	.pill-btn {
		width: 38px;
		height: 38px;
		min-width: 38px;
		min-height: 0;
		max-width: 38px;
		max-height: 38px;
		aspect-ratio: 1 / 1;
		border-radius: 50%;
		background: transparent;
		border: none;
		padding: 0;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		box-sizing: border-box;
	}

	.btn-surface {
		width: 38px;
		height: 38px;
		min-width: 38px;
		min-height: 0;
		aspect-ratio: 1 / 1;
		border-radius: 50%;
		background: none;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		box-sizing: border-box;
		transition:
			background 0.28s cubic-bezier(0.34, 1.56, 0.64, 1),
			transform 0.28s cubic-bezier(0.34, 1.56, 0.64, 1),
			box-shadow 0.25s ease;
	}

	.pill-btn:hover .btn-surface:not(.active-surface) {
		background: rgba(255, 255, 255, 0.15);
		transform: scale(1.06);
	}

	.pill-btn:active .btn-surface:not(.active-surface) {
		transform: scale(0.92);
	}

	.btn-surface.active-surface {
		background: #ffffff;
		box-shadow: 0 2px 12px rgba(0, 0, 0, 0.35);
		transform: scale(1);
	}

	.pill-btn:hover .btn-surface.active-surface {
		background: #ffffff;
		box-shadow: 0 2px 14px rgba(0, 0, 0, 0.4);
		transform: scale(1.04);
	}

	/* Icons: dim when inactive, black when active */
	.tab-icon {
		object-fit: contain;
		transition:
			filter 0.2s ease,
			opacity 0.2s ease;
		filter: brightness(0) invert(1);
		opacity: 0.7;
	}

	.tab-icon.active-icon {
		filter: brightness(0) invert(0);
		opacity: 1;
	}

	.pill-btn:hover .tab-icon:not(.active-icon) {
		filter: brightness(0) invert(1);
		opacity: 1;
	}

	.logo-icon {
		width: 20px;
		height: 14px;
	}
	.sub-icon {
		width: 17px;
		height: 17px;
	}
	.srv-icon {
		width: 17px;
		height: 17px;
	}
	.rules-icon {
		width: 18px;
		height: 18px;
	}

	/* Right slot */
	.right-slot {
		display: flex;
		align-items: center;
		justify-content: center;
	}

	/* Mode switcher pill (global/rules) */
	.action-pill {
		width: 90px;
		height: 50px;
		border-radius: 124px;
		background: rgba(0, 0, 0, 0.28);
		box-shadow:
			0 8px 32px rgba(0, 0, 0, 0.45),
			0 2px 8px rgba(0, 0, 0, 0.3);
		border: none;
		outline: none;
		-webkit-backdrop-filter: blur(16px);
		backdrop-filter: blur(16px);
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		padding: 5px 6px;
		box-sizing: border-box;
		cursor: pointer;
	}

	.action-pill.disabled {
		cursor: not-allowed;
		opacity: 0.62;
	}

	.action-pill.disabled .action-pill-btn {
		cursor: not-allowed;
	}

	.action-pill-btn {
		width: 36px;
		height: 36px;
		min-width: 36px;
		min-height: 0;
		max-width: 36px;
		max-height: 36px;
		aspect-ratio: 1 / 1;
		border-radius: 50%;
		background: none;
		border: none;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		flex-shrink: 0;
		box-sizing: border-box;
		transition:
			background 0.28s cubic-bezier(0.34, 1.56, 0.64, 1),
			transform 0.28s cubic-bezier(0.34, 1.56, 0.64, 1),
			box-shadow 0.25s ease;
	}

	.action-pill-btn:hover:not(.action-active) {
		background: rgba(255, 255, 255, 0.15);
		transform: scale(1.06);
	}

	.action-pill-btn:active:not(.action-active) {
		transform: scale(0.92);
	}

	.action-pill-btn.action-active {
		background: #ffffff;
		box-shadow: 0 2px 12px rgba(0, 0, 0, 0.35);
		transform: scale(1);
	}

	.action-pill-btn.action-active:hover {
		background: #ffffff;
		box-shadow: 0 2px 14px rgba(0, 0, 0, 0.4);
		transform: scale(1.04);
	}

	.mode-icon {
		width: 20px;
		height: 20px;
		object-fit: contain;
		filter: brightness(0) invert(1);
		opacity: 0.7;
		transition:
			opacity 0.2s ease,
			filter 0.2s ease;
	}

	.action-pill-btn:hover .mode-icon:not(.mode-active) {
		filter: brightness(0) invert(1);
		opacity: 1;
	}

	.mode-icon.mode-active {
		filter: brightness(0) invert(0);
		opacity: 1;
	}

	@media (prefers-reduced-motion: reduce) {
		.btn-surface,
		.action-pill-btn {
			transition-duration: 0ms;
		}
	}
</style>
