<script lang="ts">
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { runtime } from '../../lib/runtime';
	import { isDesktopWindow, isMac } from '../../lib/platform';
	import { t } from '../../lib/i18n';
	import { pop } from '../../lib/motion';
	import settingsIcon from '../../assets/icons/settings.svg';

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
	class:mac={isMac}
	data-tauri-drag-region
>
	<div class="logo" data-tauri-drag-region>{title}</div>

	<div class="header-right">
		{#if onOpenSettings && showQuickActions}
			<button
				transition:pop|global={{ start: 0.8 }}
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

		<!-- macOS uses the native traffic lights instead; Android has no window. -->
		{#if showControls && !isMac && isDesktopWindow}
			<div class="controls-pill">
				<button
					class="control-btn"
					onclick={() => control('minimize')}
					aria-label={t('common.minimize')}
				>
					<!-- Inline, so hover can change the icon's own color. -->
					<svg viewBox="0 0 28 28" width="28" height="28" aria-hidden="true">
						<path
							d="M8 14H20"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
						/>
					</svg>
				</button>

				<button
					class="control-btn close"
					onclick={() => control('close')}
					aria-label={t('common.close')}
				>
					<svg viewBox="0 0 28 28" width="28" height="28" aria-hidden="true">
						<path
							d="M19 19L9 9M19 9L9 19"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
						/>
					</svg>
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
	/* Native traffic lights span x=20..80 and centre at y=36 in window pixels,
	   unaffected by UI zoom. */
	.mac {
		padding-left: calc(92px / var(--ui-zoom, 1) - 16px);
		margin-top: calc(36px / var(--ui-zoom, 1) - 36px);
	}
	.mac .logo {
		margin-left: 0;
	}
	.light-backdrop .logo {
		color: #00000080;
	}
	.header-right {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	/* Figma "top main 2": over the bright shader both buttons are 20% black;
	   "top 1": over the dimmed screens, raised white glass. */
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
		transition:
			background-color 160ms ease,
			box-shadow 160ms ease,
			transform 160ms var(--ease-spring);
	}
	/* Hover changes the icon; nothing appears behind it. */
	.header-circle-btn img {
		display: block;
		opacity: 0.8;
		transition: opacity var(--hover-duration) var(--ease-hover);
	}
	.settings-btn.active {
		background: #ffffff20;
	}
	.dark-surface .header-circle-btn {
		background: var(--design-glass-raised);
		box-shadow: var(--design-glow);
		backdrop-filter: blur(25px);
	}
	.controls-pill {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 78px;
		height: 40px;
		padding: 6px 8px;
		border-radius: 32px;
		background: var(--design-glass);
		transition:
			background-color 160ms ease,
			box-shadow 160ms ease;
	}
	.dark-surface .controls-pill {
		background: var(--design-panel);
		box-shadow: var(--design-edge), var(--design-glow);
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
		color: #ffffffcc;
		transition:
			color var(--hover-duration) var(--ease-hover),
			transform 160ms var(--ease-spring);
	}
	.control-btn svg {
		display: block;
		width: 28px;
		height: 28px;
	}
	.settings-btn.active img {
		opacity: 1;
	}
	.header-circle-btn:active,
	.control-btn:active {
		transform: scale(0.94);
	}
	@media (hover: hover) and (pointer: fine) {
		.header-circle-btn:hover img {
			opacity: 1;
		}
		.control-btn:hover {
			color: #fff;
		}
		/* The cross turns red (the app's error tint), as a close should. */
		.control-btn.close:hover {
			color: #ff4b5a;
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
