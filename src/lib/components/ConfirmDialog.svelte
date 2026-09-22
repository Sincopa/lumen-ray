<script lang="ts">
	import { onMount } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { fade, scale } from 'svelte/transition';

	interface Props {
		title: string;
		description: string;
		cancelLabel: string;
		confirmLabel: string;
		onCancel: () => void;
		onConfirm: () => void | Promise<void>;
		busy?: boolean;
	}

	let {
		title,
		description,
		cancelLabel,
		confirmLabel,
		onCancel,
		onConfirm,
		busy = false,
	}: Props = $props();

	let cancelButton: HTMLButtonElement;

	onMount(() => {
		const previous = document.activeElement as HTMLElement | null;
		cancelButton.focus();
		return () => previous?.focus();
	});

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && !busy) {
			event.preventDefault();
			onCancel();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="confirm-backdrop"
	role="presentation"
	transition:fade={{ duration: 180 }}
	onclick={(event) => {
		if (event.currentTarget === event.target && !busy) onCancel();
	}}
>
	<div
		class="confirm-dialog"
		role="dialog"
		aria-modal="true"
		aria-labelledby="confirm-dialog-title"
		aria-describedby="confirm-dialog-description"
		transition:scale={{ start: 0.96, duration: 260, easing: cubicOut }}
	>
		<div class="confirm-copy">
			<h2 id="confirm-dialog-title">{title}</h2>
			<p id="confirm-dialog-description">{description}</p>
		</div>

		<div class="confirm-actions">
			<button
				bind:this={cancelButton}
				type="button"
				class="confirm-button cancel"
				disabled={busy}
				onclick={onCancel}
			>
				{cancelLabel}
			</button>
			<button
				type="button"
				class="confirm-button continue"
				disabled={busy}
				onclick={onConfirm}
			>
				{confirmLabel}
			</button>
		</div>
	</div>
</div>

<style>
	.confirm-backdrop {
		position: fixed;
		left: 50%;
		top: 50%;
		width: min(var(--app-width), 100vw);
		height: min(var(--app-height), 100dvh);
		transform: translate(-50%, -50%);
		z-index: 80;
		display: grid;
		place-items: center;
		padding: 32px;
		overflow: hidden;
		border-radius: var(--radius-window) var(--radius-window)
			var(--radius-window-bottom) var(--radius-window-bottom);
		background: rgba(54, 54, 54, 0.75);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}

	:global(body:has(.confirm-backdrop) .window-header) {
		pointer-events: none;
	}

	.confirm-dialog {
		width: min(326px, 100%);
		display: flex;
		flex-direction: column;
		gap: 48px;
		color: #fff;
		transform-origin: 50% 54%;
	}

	.confirm-copy {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
		text-align: center;
	}

	h2 {
		width: 100%;
		margin: 0;
		font-size: 32px;
		font-weight: 500;
		line-height: 1;
		letter-spacing: 0;
	}

	p {
		width: 100%;
		margin: 0;
		color: rgba(255, 255, 255, 0.75);
		font-size: 14px;
		font-weight: 400;
		line-height: 1.4;
	}

	.confirm-actions {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.confirm-button {
		width: 100%;
		height: 64px;
		min-height: 64px;
		border: 0;
		border-radius: 123px;
		font-size: 18px;
		font-weight: 600;
		letter-spacing: -0.36px;
		transition:
			transform 180ms var(--ease-spring),
			background-color 180ms ease,
			opacity 180ms ease;
	}

	.confirm-button:hover {
		transform: scale(1.015);
	}

	.confirm-button:active {
		transform: scale(0.985);
	}

	.confirm-button.cancel {
		background: rgba(255, 255, 255, 0.15);
		color: rgba(255, 255, 255, 0.8);
	}

	.confirm-button.cancel:hover {
		background: rgba(255, 255, 255, 0.2);
	}

	.confirm-button.continue {
		background: #fff;
		color: rgba(0, 0, 0, 0.8);
	}

	.confirm-button.continue:hover {
		background: #f4f4f4;
	}

	.confirm-button:disabled {
		opacity: 0.55;
		transform: none;
	}
</style>
