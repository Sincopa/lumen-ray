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
	/** The live layer for each key, including layers that are dissolving.
	 *  Bookkeeping only: the effect below reruns on `key`, not on this map. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const layers = new Map<unknown, HTMLElement>();

	function fit(node: HTMLElement) {
		width = node.offsetWidth;
		height = node.offsetHeight;
		if (!ready) requestAnimationFrame(() => (ready = true));
	}

	function measure(node: HTMLElement, layerKey: unknown) {
		layers.set(layerKey, node);
		const observer = new ResizeObserver(() => {
			// A leaving layer must not pull the box back to its size.
			if (layerKey === key) fit(node);
		});
		observer.observe(node);
		return {
			destroy: () => {
				observer.disconnect();
				if (layers.get(layerKey) === node) layers.delete(layerKey);
			},
		};
	}

	// A key that returns while its layer is still dissolving (Tun, +, Tun in
	// quick succession) gets that same layer back. Its size has not changed, so
	// the observer stays quiet and the box kept the other layer's size.
	$effect(() => {
		const node = layers.get(key);
		if (node) fit(node);
	});
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
