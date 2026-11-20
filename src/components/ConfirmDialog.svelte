<script lang="ts">
	import backIcon from '../assets/icons/back.svg';
	import trashIcon from '../assets/icons/trash.svg';
	import { onMount } from 'svelte';
	import DesignOverlay from './DesignOverlay.svelte';

	interface Props {
		title: string;
		description: string;
		cancelLabel: string;
		confirmLabel: string;
		onCancel: () => void;
		onConfirm: () => void | Promise<void>;
		busy?: boolean;
		/** Deletions use the round back and trash buttons. */
		destructive?: boolean;
	}

	let {
		title,
		description,
		cancelLabel,
		confirmLabel,
		onCancel,
		onConfirm,
		busy = false,
		destructive = false,
	}: Props = $props();

	let cancelButton = $state<HTMLButtonElement>();

	onMount(() => {
		const previous = document.activeElement as HTMLElement | null;
		cancelButton?.focus();
		return () => previous?.focus();
	});
</script>

<DesignOverlay
	labelledby="confirm-dialog-title"
	describedby="confirm-dialog-description"
	panel={!destructive}
	dismissible={!busy}
	onDismiss={onCancel}
>
	<div class="confirm-dialog">
		<div class="confirm-copy">
			<h2 id="confirm-dialog-title">{title}</h2>
			<p id="confirm-dialog-description">{description}</p>
		</div>

		{#if !destructive}
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
		{/if}
	</div>
	{#snippet footer()}
		{#if destructive}
			<!-- Figma "delete screen": back on the left, the red bin on the right,
			     on the row of the bottom navigation. -->
			<div class="confirm-actions round">
				<button
					bind:this={cancelButton}
					type="button"
					class="round-button cancel"
					aria-label={cancelLabel}
					disabled={busy}
					onclick={onCancel}
				>
					<img src={backIcon} alt="" width="32" height="32" />
				</button>
				<button
					type="button"
					class="round-button delete"
					aria-label={confirmLabel}
					disabled={busy}
					onclick={onConfirm}
				>
					<img src={trashIcon} alt="" width="34" height="34" />
				</button>
			</div>
		{/if}
	{/snippet}
</DesignOverlay>

<style>
	.confirm-dialog {
		display: flex;
		flex-direction: column;
		gap: 48px;
	}

	.confirm-copy {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
		padding: 0 12px;
		text-align: center;
	}

	h2 {
		width: 100%;
		margin: 0;
		font-size: 32px;
		font-weight: 400;
		line-height: 32px;
		letter-spacing: 0;
	}

	p {
		width: 100%;
		margin: 0;
		color: #ffffff80;
		font-size: 16px;
		font-weight: 400;
		line-height: 20.8px;
		letter-spacing: -0.32px;
	}

	.confirm-actions {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.confirm-actions.round {
		align-self: flex-end;
		flex-direction: row;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		padding: 0 calc(var(--nav-inset-end) + 8px) 0 var(--nav-inset);
	}

	.round-button {
		display: grid;
		place-items: center;
		width: 64px;
		height: 64px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transition:
			transform 180ms var(--ease-spring),
			background-color 180ms ease,
			opacity 180ms ease;
	}
	.round-button.cancel {
		background: var(--design-glass-raised);
		box-shadow: var(--design-glow);
	}
	/* Solid red over 20% black with a 15% white rim. */
	.round-button.delete {
		background: #d03441;
		box-shadow: inset 0 0 0 1px #ffffff26;
	}
	.round-button:active:not(:disabled) {
		transform: scale(0.94);
	}

	.confirm-button {
		width: 100%;
		height: 64px;
		min-height: 64px;
		border: 0;
		border-radius: 123px;
		font-size: 18px;
		font-weight: 600;
		line-height: 18px;
		letter-spacing: -0.4px;
		transition:
			transform 180ms var(--ease-spring),
			background-color 180ms ease,
			opacity 180ms ease;
	}
	.confirm-button:active:not(:disabled) {
		transform: scale(0.985);
	}
	.confirm-button.cancel {
		background: var(--design-glass-raised);
		box-shadow: var(--design-glow);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		color: #fff;
	}
	.confirm-button.continue {
		background: #fff;
		color: #000000cc;
	}

	.round-button:disabled,
	.confirm-button:disabled {
		opacity: 0.55;
		transform: none;
	}

	@media (hover: hover) and (pointer: fine) {
		.round-button.cancel:hover:not(:disabled),
		.confirm-button.cancel:hover:not(:disabled) {
			background: #ffffff1a;
		}
		.round-button.delete:hover:not(:disabled) {
			background: #e03d4b;
		}
		.confirm-button.continue:hover:not(:disabled) {
			background: #f4f4f4;
		}
	}
</style>
