<script lang="ts">
	import type { Snippet } from 'svelte';
	import { morphIn, morphOut } from '../lib/motion';

	/*
	 * A box that morphs between contents. When `key` changes, the old content
	 * dissolves through a blur, the new one appears, and the box springs from
	 * the old size to the new one. Size changes within one content (a ticking
	 * timer) spring the same way. The parent supplies the shape around it.
	 */
	interface Props {
		key: unknown;
		/** Where the content sits while the box resizes. */
		anchor?: 'start' | 'center' | 'end';
		class?: string;
		children: Snippet;
	}

	let {
		key,
		anchor = 'center',
		class: className = '',
		children,
	}: Props = $props();

	let width = $state<number | null>(null);
	let height = $state<number | null>(null);
	let ready = $state(false);

	function measure(node: HTMLElement, layerKey: unknown) {
		const observer = new ResizeObserver(() => {
			// A leaving layer must not pull the box back to its size.
			if (layerKey !== key) return;
			width = node.offsetWidth;
			height = node.offsetHeight;
			if (!ready) requestAnimationFrame(() => (ready = true));
		});
		observer.observe(node);
		return { destroy: () => observer.disconnect() };
	}
</script>

<span
	class="morph anchor-{anchor} {className}"
	class:ready
	style:width={width === null ? undefined : `${width}px`}
	style:height={height === null ? undefined : `${height}px`}
>
	{#key key}
		<span class="morph-layer" use:measure={key} in:morphIn out:morphOut>
			{@render children()}
		</span>
	{/key}
</span>

<style>
	/* Layers overlap while one dissolves into the next; the box carries the size. */
	.morph {
		position: relative;
		display: block;
		flex-shrink: 0;
	}
	.morph.ready {
		transition:
			width var(--morph-duration) var(--ease-morph),
			height var(--morph-duration) var(--ease-morph);
	}
	.morph-layer {
		position: absolute;
		top: 0;
		bottom: 0;
		display: flex;
		align-items: center;
		width: max-content;
		height: max-content;
		margin-block: auto;
	}
	.anchor-start .morph-layer {
		left: 0;
		transform-origin: 30% 50%;
	}
	.anchor-end .morph-layer {
		right: 0;
		transform-origin: 70% 50%;
	}
	.anchor-center .morph-layer {
		left: 0;
		right: 0;
		margin-inline: auto;
	}
	@media (prefers-reduced-motion: reduce) {
		.morph.ready {
			transition: none;
		}
	}
</style>
