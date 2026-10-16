<script lang="ts">
	import type { Snippet } from 'svelte';
	import { tooltip } from '../actions/tooltip';

	interface Props {
		icon?: string;
		design?: boolean;
		ariaLabel?: string;
		title?: string;
		size?: 'sm' | 'md' | 'lg';
		variant?: 'ghost' | 'surface' | 'primary' | 'danger';
		active?: boolean;
		disabled?: boolean;
		expanded?: boolean;
		type?: 'button' | 'submit' | 'reset';
		class?: string;
		onclick?: (e: MouseEvent) => void;
		children?: Snippet;
	}

	let {
		icon,
		design = false,
		ariaLabel,
		title,
		size = 'sm',
		variant = 'ghost',
		active = false,
		disabled = false,
		expanded,
		type = 'button',
		class: className = '',
		onclick,
		children,
	}: Props = $props();

	// Tooltip text. `title` wins, then `ariaLabel`.
	let tipLabel = $derived(title ?? ariaLabel ?? '');
</script>

<button
	{type}
	class="icon-btn size-{size} variant-{variant} {className}"
	class:active
	class:design
	class:expanded
	aria-label={ariaLabel}
	aria-expanded={expanded}
	use:tooltip={tipLabel}
	{disabled}
	{onclick}
>
	{#if children}
		{@render children()}
	{:else if icon}
		<img src={icon} alt="" class="btn-icon" />
	{/if}
</button>

<style>
	.icon-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		border: 0;
		padding: 0;
		cursor: pointer;
		flex-shrink: 0;
		box-sizing: border-box;
		transition:
			background-color 180ms ease,
			transform 200ms var(--ease-spring),
			border-color 180ms ease,
			opacity 180ms ease;
	}

	.icon-btn:disabled {
		opacity: 0.45;
		cursor: not-allowed;
		transform: none !important;
	}

	/* Sizes */
	.size-sm {
		width: 30px;
		height: 30px;
		min-width: 30px;
		min-height: 0;
	}
	.size-sm .btn-icon {
		width: 18px;
		height: 18px;
	}

	.size-md {
		width: 36px;
		height: 36px;
		min-width: 36px;
		min-height: 0;
	}
	.size-md .btn-icon {
		width: 18px;
		height: 18px;
	}

	.size-lg {
		width: 50px;
		height: 50px;
		min-width: 50px;
		min-height: 0;
	}
	.size-lg .btn-icon {
		width: 22px;
		height: 22px;
	}

	/* Variants */
	.variant-ghost {
		background: transparent;
	}
	.variant-ghost:hover:not(:disabled),
	.variant-ghost.active {
		background: rgba(255, 255, 255, 0.15);
	}

	.variant-surface {
		background: rgba(0, 0, 0, 0.28);
		border: none;
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
	}
	.variant-surface:hover:not(:disabled),
	.variant-surface.active {
		background: rgba(255, 255, 255, 0.14);
		transform: scale(1.04);
	}

	.variant-primary {
		background: #ffffff;
		box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
	}
	.variant-primary:hover:not(:disabled) {
		transform: scale(1.05);
		background: #f5f5f5;
	}
	.variant-primary .btn-icon {
		filter: brightness(0);
	}

	.variant-danger {
		background: #d42a2a;
		box-shadow: 0 2px 10px rgba(212, 42, 42, 0.35);
	}
	.variant-danger:hover:not(:disabled) {
		background: #e23333;
		transform: scale(1.05);
	}
	.variant-danger .btn-icon {
		filter: brightness(0) invert(1);
	}

	.icon-btn:active:not(:disabled) {
		transform: scale(0.92);
	}

	.btn-icon {
		object-fit: contain;
		filter: brightness(0) invert(1);
		opacity: 0.8;
		transition:
			opacity 180ms ease,
			transform 200ms ease;
	}

	.icon-btn:hover:not(:disabled) .btn-icon,
	.icon-btn.active .btn-icon {
		opacity: 1;
	}

	@media (prefers-reduced-motion: reduce) {
		.icon-btn {
			transition-duration: 0ms;
		}
	}
	.design.size-sm {
		width: 28px;
		height: 28px;
		min-width: 28px;
	}
	.design .btn-icon {
		width: auto;
		height: auto;
		max-width: 100%;
		max-height: 100%;
		filter: none;
		opacity: 1;
	}
	.design.variant-surface {
		background: var(--design-glass-dark);
		box-shadow: none;
		backdrop-filter: blur(25px);
	}
	.design.variant-primary {
		box-shadow: none;
	}
	.design.variant-primary .btn-icon {
		filter: brightness(0);
	}
	/* An unavailable confirm keeps its shape, dimmed. */
	.design.variant-primary:disabled {
		background: #262626;
		opacity: 1;
	}
	.design.variant-primary:disabled .btn-icon {
		filter: brightness(0) invert(1);
		opacity: 0.5;
	}
</style>
