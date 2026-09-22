<script lang="ts">
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { runtime } from '../runtime';
	import { t } from '../i18n';
	import minimizeIcon from '../../assets/minimaze.svg';
	import maximizeIcon from '../../assets/maximaze.svg';
	import crossIcon from '../../assets/cross.svg';
	import settingsIcon from '../../assets/settings.svg';
	import bellIcon from '../../assets/bell-ring.svg';

	interface Props {
		title?: string;
		showControls?: boolean;
		darkSurface?: boolean;
		isSettingsOpen?: boolean;
		onOpenSettings?: () => void;
		updateVersion?: string | null;
		onOpenUpdate?: () => void;
	}

	let {
		title = 'Lumen',
		showControls = true,
		darkSurface = false,
		isSettingsOpen = false,
		onOpenSettings,
		updateVersion = null,
		onOpenUpdate,
	}: Props = $props();

	let failure = $state(false);

	async function control(action: 'minimize' | 'toggleMaximize' | 'close') {
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
	data-tauri-drag-region
>
	<div class="logo">{title}</div>

	<div class="header-right">
		{#if updateVersion && onOpenUpdate}
			<button
				class="header-circle-btn update-btn"
				onclick={onOpenUpdate}
				aria-label={t('settings.updateAvailable', { version: updateVersion })}
				title={t('settings.updateAvailable', { version: updateVersion })}
			>
				<img src={bellIcon} alt="" width="24" height="24" />
				<span class="update-dot" aria-hidden="true"></span>
			</button>
		{/if}

		{#if onOpenSettings}
			<button
				class="header-circle-btn settings-btn"
				class:active={isSettingsOpen}
				onclick={onOpenSettings}
				aria-label={t('settings.title')}
			>
				<img
					src={settingsIcon}
					alt=""
					width="24"
					height="24"
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

				<button
					class="control-btn"
					onclick={() => control('toggleMaximize')}
					aria-label={t('common.maximize')}
				>
					<img src={maximizeIcon} alt="" width="21" height="21" />
				</button>

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
		font-family: var(--font-family-base);
		font-weight: 700;
		font-size: 24px;
		color: #66666a;
		letter-spacing: -0.5px;
		margin-left: 8px;
		user-select: none;
	}

	.dark-surface .logo {
		color: rgba(255, 255, 255, 0.56);
	}

	.header-right {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.header-circle-btn {
		width: 40px;
		height: 40px;
		min-width: 40px;
		min-height: 40px;
		border-radius: 50%;
		background: rgba(0, 0, 0, 0.25);
		border: none;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		box-sizing: border-box;
		backdrop-filter: blur(10px);
		transition:
			background 0.2s ease,
			transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
	}

	.header-circle-btn:hover {
		background: rgba(255, 255, 255, 0.15);
		transform: scale(1.08);
	}

	.header-circle-btn:active {
		transform: scale(0.92);
	}

	.settings-btn.active {
		background: rgba(255, 255, 255, 0.25);
	}

	.update-btn {
		position: relative;
		animation: bell-arrival 420ms var(--ease-spring);
	}

	.update-btn img {
		filter: brightness(0) invert(1);
		opacity: 0.82;
	}

	.update-dot {
		position: absolute;
		top: 5px;
		right: 5px;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-accent);
		box-shadow: 0 0 0 2px rgba(20, 20, 20, 0.82);
	}

	@keyframes bell-arrival {
		0% {
			transform: scale(0.65) rotate(-14deg);
			opacity: 0;
		}
		100% {
			transform: scale(1) rotate(0);
			opacity: 1;
		}
	}

	.settings-icon {
		filter: brightness(0) invert(1);
		opacity: 0.8;
		transition: opacity 0.2s ease;
	}

	.settings-btn:hover .settings-icon,
	.settings-btn.active .settings-icon {
		opacity: 1;
	}

	.controls-pill {
		display: flex;
		align-items: center;
		background: rgba(0, 0, 0, 0.2);
		border: none;
		border-radius: 123px;
		width: 112px;
		height: 40px;
		padding: 6px;
		gap: 0;
		justify-content: space-between;
		box-sizing: border-box;
		backdrop-filter: blur(10px);
	}

	.control-btn {
		width: 28px;
		height: 28px;
		min-width: 28px;
		min-height: 0;
		max-width: 28px;
		max-height: 28px;
		aspect-ratio: 1 / 1;
		flex-shrink: 0;
		border-radius: 50%;
		background: transparent;
		border: none;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		box-sizing: border-box;
		transition:
			background 0.2s ease,
			transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
	}

	.control-btn:hover {
		background: rgba(255, 255, 255, 0.15);
		transform: scale(1.1);
	}

	.control-btn:active {
		transform: scale(0.92);
	}

	.control-btn.close:hover {
		background: rgba(243, 57, 57, 0.6);
	}

	@media (prefers-reduced-motion: reduce) {
		.update-btn {
			animation: none;
		}
	}
</style>
