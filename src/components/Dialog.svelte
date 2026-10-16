<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { fade, scale } from 'svelte/transition';
	import { t } from '../lib/i18n';

	interface Props {
		title: string;
		onClose: () => void;
		children: Snippet;
		variant?: 'panel' | 'immersive';
	}

	let { title, onClose, children, variant = 'panel' }: Props = $props();
	let closeButton: HTMLButtonElement;

	onMount(() => {
		const previous = document.activeElement as HTMLElement | null;
		closeButton.focus();
		return () => previous?.focus();
	});

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="dialog-backdrop"
	class:immersive={variant === 'immersive'}
	role="presentation"
	transition:fade={{ duration: 180 }}
	onclick={(event) => {
		if (event.currentTarget === event.target) onClose();
	}}
>
	<div
		class="dialog-panel"
		class:immersive={variant === 'immersive'}
		role="dialog"
		aria-modal="true"
		aria-labelledby="dialog-title"
		transition:scale={{ start: 0.96, duration: 240, easing: cubicOut }}
	>
		<header>
			<h2 id="dialog-title">{title}</h2>
			<button
				bind:this={closeButton}
				type="button"
				onclick={onClose}
				aria-label={t('common.close')}
			>
				×
			</button>
		</header>
		{@render children()}
	</div>
</div>

<style>
	.dialog-backdrop {
		position: fixed;
		left: 50%;
		top: 50%;
		width: min(var(--app-width), 100vw);
		height: min(var(--app-height), 100dvh);
		transform: translate(-50%, -50%);
		z-index: var(--z-modal);
		display: grid;
		place-items: center;
		padding: 16px;
		overflow: hidden;
		border-radius: var(--radius-window) var(--radius-window)
			var(--radius-window-bottom) var(--radius-window-bottom);
		background: rgba(0, 0, 0, 0.58);
		-webkit-backdrop-filter: blur(10px);
		backdrop-filter: blur(10px);
	}

	.dialog-backdrop.immersive {
		z-index: 80;
		padding: 32px;
		background: rgba(54, 54, 54, 0.75);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}

	:global(body:has(.dialog-backdrop.immersive) .window-header) {
		pointer-events: none;
	}

	.dialog-panel {
		width: min(360px, calc(100vw - 32px));
		max-height: calc(100dvh - 32px);
		overflow: auto;
		border: none;
		border-radius: 30px;
		padding: 22px;
		background: #151516;
		color: #fff;
		box-shadow: 0 24px 70px #000b;
		transform-origin: center;
	}

	.dialog-panel.immersive {
		width: min(326px, 100%);
		max-height: 100%;
		overflow: visible;
		border-radius: 0;
		padding: 0;
		background: transparent;
		box-shadow: none;
		transform-origin: 50% 54%;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin-bottom: 18px;
	}

	.dialog-panel.immersive header {
		position: relative;
		justify-content: center;
		margin-bottom: 16px;
		padding: 0 44px;
		text-align: center;
	}

	h2 {
		font-size: 20px;
		line-height: 1.2;
		letter-spacing: -0.4px;
		margin: 0;
	}

	.dialog-panel.immersive h2 {
		width: 100%;
		font-size: 28px;
		font-weight: 500;
		line-height: 1.05;
		letter-spacing: -0.4px;
	}

	button {
		width: 36px;
		height: 36px;
		min-height: 0;
		flex-shrink: 0;
		border: none;
		border-radius: 50%;
		background: #ffffff10;
		padding: 0;
		font-size: 23px;
		line-height: 1;
	}

	.dialog-panel.immersive header button {
		position: absolute;
		right: 0;
		top: 50%;
		transform: translateY(-50%);
		background: rgba(255, 255, 255, 0.15);
		color: rgba(255, 255, 255, 0.8);
		transition:
			transform 180ms var(--ease-spring),
			background-color 180ms ease;
	}

	.dialog-panel.immersive header button:hover {
		transform: translateY(-50%) scale(1.05);
		background: rgba(255, 255, 255, 0.22);
	}

	.dialog-panel.immersive header button:active {
		transform: translateY(-50%) scale(0.94);
	}
</style>
