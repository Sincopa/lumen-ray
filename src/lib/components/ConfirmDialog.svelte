<script lang="ts">
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
		/** Deletions use the round back / trash pair from Figma 202:7009. */
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
	panel
	dismissible={!busy}
	onDismiss={onCancel}
>
	<div class="confirm-dialog">
		<div class="confirm-copy">
			<h2 id="confirm-dialog-title">{title}</h2>
			<p id="confirm-dialog-description">{description}</p>
		</div>

		{#if destructive}
			<div class="confirm-actions round">
				<button
					bind:this={cancelButton}
					type="button"
					class="round-button cancel"
					aria-label={cancelLabel}
					disabled={busy}
					onclick={onCancel}
				>
					<svg
						width="32"
						height="32"
						viewBox="0 0 32 32"
						fill="none"
						aria-hidden="true"
					>
						<path
							d="M18.6 9.2 12.4 15.4a.85.85 0 0 0 0 1.2l6.2 6.2"
							stroke="#fff"
							stroke-width="1.6"
							stroke-linecap="round"
						/>
					</svg>
				</button>
				<button
					type="button"
					class="round-button delete"
					aria-label={confirmLabel}
					disabled={busy}
					onclick={onConfirm}
				>
					<svg
						width="32"
						height="32"
						viewBox="0 0 24 24"
						fill="none"
						aria-hidden="true"
					>
						<path
							d="M7.28 10.26a.38.38 0 0 1 .38-.44h8.79c.23 0 .41.2.38.44l-.98 7.59a1.55 1.55 0 0 1-1.53 1.35H9.79a1.55 1.55 0 0 1-1.53-1.35l-.98-7.59Z"
							fill="#b1313b"
						/>
						<path
							d="M7.88 6.04a.38.38 0 0 0 .22.57l8.31 2.4a.38.38 0 0 0 .49-.37l.01-.71a.78.78 0 0 0-.56-.75L9.13 5.1a.78.78 0 0 0-.87.34l-.38.6Z"
							fill="#b1313b"
						/>
					</svg>
				</button>
			</div>
		{:else}
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
		font-weight: 500;
		line-height: 32px;
		letter-spacing: 0;
	}

	p {
		width: 100%;
		margin: 0;
		color: #ffffff80;
		font-size: 14px;
		font-weight: 400;
		line-height: 18.2px;
		letter-spacing: -0.3px;
	}

	.confirm-actions {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.confirm-actions.round {
		flex-direction: row;
		justify-content: center;
		gap: 30px;
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
		background: #ffffff0d;
	}
	.round-button.delete {
		background: #d0344126;
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
		background: #ffffff0d;
		color: #ffffffcc;
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
			background: #d0344140;
		}
		.confirm-button.continue:hover:not(:disabled) {
			background: #f4f4f4;
		}
	}
</style>
