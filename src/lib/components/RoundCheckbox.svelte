<script lang="ts">
	interface Props {
		checked: boolean;
		ariaLabel?: string;
		disabled?: boolean;
		asSpan?: boolean;
		class?: string;
		onclick?: (e: MouseEvent) => void;
	}

	let {
		checked,
		ariaLabel,
		disabled = false,
		asSpan = false,
		class: className = '',
		onclick,
	}: Props = $props();
</script>

{#if asSpan}
	<span
		class="select-checkbox {className}"
		class:checked
		aria-hidden="true"
	></span>
{:else}
	<button
		type="button"
		class="select-checkbox {className}"
		class:checked
		role="checkbox"
		aria-checked={checked}
		aria-label={ariaLabel}
		{disabled}
		{onclick}
	></button>
{/if}

<style>
	/* Figma "Component 26" — glass well with a white knob when checked. */
	.select-checkbox {
		display: inline-grid;
		place-items: center;
		flex-shrink: 0;
		width: 28px;
		height: 28px;
		min-width: 28px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: #ffffff0a;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		cursor: pointer;
		transition: background-color 160ms ease;
	}
	.select-checkbox::after {
		content: '';
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: #fff;
		box-shadow: 0 2px 7.7px #00000040;
		transform: scale(0);
		transition: transform 180ms var(--ease-spring);
	}
	.select-checkbox.checked::after {
		transform: scale(1);
	}
	.select-checkbox:hover:not(:disabled) {
		background: #ffffff14;
	}
	.select-checkbox:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
