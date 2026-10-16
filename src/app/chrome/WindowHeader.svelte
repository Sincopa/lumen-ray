<script lang="ts">
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { runtime } from '../../lib/runtime';
	import { t } from '../../lib/i18n';
	import minimizeIcon from '../../assets/minimize.svg';
	import maximizeIcon from '../../assets/maximize.svg';
	import crossIcon from '../../assets/close.svg';
	import settingsIcon from '../../assets/settings.svg';

	interface Props {
		title?: string;
		showControls?: boolean;
		darkSurface?: boolean;
		/** The shader behind the logo is bright, so the logo turns dark. */
		lightBackdrop?: boolean;
		showQuickActions?: boolean;
		isSettingsOpen?: boolean;
		onOpenSettings?: () => void;
	}

	let {
		title = 'Lumen',
		showControls = true,
		darkSurface = false,
		lightBackdrop = false,
		showQuickActions = true,
		isSettingsOpen = false,
		onOpenSettings,
	}: Props = $props();

	let failure = $state(false);

	async function control(action: 'minimize' | 'close') {
		if (!runtime.isTauri) return;
		try {
			await getCurrentWindow()[action]();
		} catch {
			failure = true;
		}
	}
</script>

<header
	class="window-header"
	class:dark-surface={darkSurface}
	class:light-backdrop={lightBackdrop && !darkSurface}
	data-tauri-drag-region
>
	<div class="logo">{title}</div>

	<div class="header-right">
		{#if onOpenSettings && showQuickActions}
			<button
				class="header-circle-btn settings-btn"
				class:active={isSettingsOpen}
				onclick={onOpenSettings}
				aria-label={t('settings.title')}
			>
				<img
					src={settingsIcon}
					alt=""
					width="32"
					height="32"
					class="settings-icon"
				/>
			</button>
		{/if}

		{#if showControls}
			<div class="controls-pill">
				<button
					class="control-btn"
					onclick={() => control('minimize')}
					aria-label={t('common.minimize')}
				>
					<img src={minimizeIcon} alt="" width="21" height="21" />
				</button>

				<!-- Maximize is shown for the layout only; the window keeps its size. -->
				<span class="control-btn inert" aria-hidden="true">
					<img src={maximizeIcon} alt="" width="21" height="21" />
				</span>

				<button
					class="control-btn close"
					onclick={() => control('close')}
					aria-label={t('common.close')}
				>
					<img src={crossIcon} alt="" width="21" height="21" />
				</button>
			</div>
		{/if}
	</div>
</header>

{#if failure}
	<p role="alert">{t('errors.unavailable')}</p>
{/if}

<style>
	.window-header {
		position: relative;
		z-index: 90;
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		height: 40px;
		margin-bottom: 16px;
		flex-shrink: 0;
	}
	.logo {
		font-weight: 700;
		font-size: 24px;
		line-height: 1;
		/* Follows the shader under it: light grey on dark, dark grey on bright. */
		color: #ffffff80;
		letter-spacing: -1.68px;
		margin-left: 9px;
		user-select: none;
		transition: color 600ms ease;
	}
	.light-backdrop .logo {
		color: #00000080;
	}
	.header-right {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.header-circle-btn {
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: var(--design-glass);
		backdrop-filter: blur(25px);
		transition:
			background-color 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.header-circle-btn img {
		display: block;
	}
	.settings-btn.active {
		background: #ffffff20;
	}
	.dark-surface .controls-pill,
	.dark-surface .header-circle-btn {
		background: var(--design-glass-dark);
	}
	.controls-pill {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 112px;
		height: 40px;
		padding: 6px;
		border-radius: 123px;
		background: var(--design-glass);
		backdrop-filter: blur(25px);
	}
	.control-btn {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		transition:
			background-color 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.control-btn img {
		width: 28px;
		height: 28px;
	}
	.header-circle-btn:active,
	.control-btn:not(.inert):active {
		transform: scale(0.94);
	}
	@media (hover: hover) and (pointer: fine) {
		.header-circle-btn:hover,
		.control-btn:not(.inert):hover {
			background: #ffffff20;
		}
		.control-btn.close:hover {
			background: #f3393980;
		}
	}
	@media (max-width: 350px) {
		.header-right {
			gap: 6px;
		}
	}
	@media (max-width: 310px) {
		.header-right {
			gap: 4px;
		}
		.logo {
			font-size: 22px;
			letter-spacing: -1.54px;
			margin-left: 4px;
		}
	}
</style>
