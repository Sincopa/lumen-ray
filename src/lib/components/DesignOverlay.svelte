<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fade } from 'svelte/transition';
	import { pop, rise } from '../motion';

	interface Props {
		labelledby: string;
		describedby?: string;
		/** Figma 202:7009 draws a faint inner panel behind the content. */
		panel?: boolean;
		dismissible?: boolean;
		onDismiss: () => void;
		children: Snippet;
		footer?: Snippet;
	}

	let {
		labelledby,
		describedby,
		panel = false,
		dismissible = true,
		onDismiss,
		children,
		footer,
	}: Props = $props();

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && dismissible) {
			event.preventDefault();
			onDismiss();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Figma 202:7009 / 202:6372 — full-window dim with the header left on top. -->
<div
	class="design-overlay"
	role="presentation"
	in:fade={{ duration: 200 }}
	out:fade={{ duration: 160 }}
	onclick={(event) => {
		if (event.currentTarget === event.target && dismissible) onDismiss();
	}}
>
	{#if panel}<div
			class="overlay-panel"
			aria-hidden="true"
			in:pop={{ start: 0.98, duration: 280 }}
		></div>{/if}
	<div
		class="overlay-content"
		role="dialog"
		aria-modal="true"
		aria-labelledby={labelledby}
		aria-describedby={describedby}
		in:pop={{ start: 0.95, duration: 300, delay: 40 }}
		out:pop={{ start: 0.97, duration: 140 }}
	>
		{@render children()}
	</div>
	{#if footer}
		<div class="overlay-footer" in:rise={{ delay: 60 }}>
			{@render footer()}
		</div>
	{/if}
</div>

<style>
	.design-overlay {
		position: fixed;
		left: 50%;
		top: 50%;
		z-index: 80;
		display: grid;
		place-items: center;
		width: min(var(--app-width), 100vw);
		height: min(var(--app-height), 100dvh);
		padding: 32px;
		overflow: hidden;
		border-radius: var(--radius-window) var(--radius-window)
			var(--radius-window-bottom) var(--radius-window-bottom);
		background: #000000bf;
		/* Figma's background blur radius is twice the CSS blur value. */
		-webkit-backdrop-filter: blur(12.5px);
		backdrop-filter: blur(12.5px);
		transform: translate(-50%, -50%);
	}
	:global(body:has(.design-overlay) .window-header .header-circle-btn) {
		pointer-events: none;
	}
	.overlay-panel {
		position: absolute;
		inset: 73px 11px 11px;
		border-radius: 22px 22px 44px 44px;
		background: #ffffff0a;
		pointer-events: none;
	}
	.overlay-content {
		position: relative;
		width: min(326px, 100%);
		color: #fff;
		transform-origin: 50% 54%;
	}
	.overlay-footer {
		position: absolute;
		left: 0;
		right: 0;
		bottom: var(--nav-bottom);
		display: flex;
		justify-content: center;
		align-items: center;
		height: var(--nav-height);
	}
</style>
