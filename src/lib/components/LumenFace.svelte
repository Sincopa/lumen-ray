<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { AnimationItem } from 'lottie-web';
	import faceIcon from '../../assets/lumen-face.svg';
	import type { VpnConnectionState } from '../types/models';

	let {
		status = 'disconnected',
		hovered = false,
		error = false,
	}: {
		status?: VpnConnectionState;
		hovered?: boolean;
		error?: boolean;
	} = $props();
	let container: HTMLDivElement;
	let animation = $state.raw<AnimationItem | null>(null);
	let ready = $state(false);
	let reducedMotion = $state(false);
	let visible = $state(true);
	type Expression =
		'idle' | 'connecting' | 'connected' | 'wink' | 'error' | 'disconnecting';
	const segments = new SvelteMap<
		string,
		{ start: number; end: number; loop: boolean; next?: string }
	>();
	let expression: Expression = $derived(
		error
			? 'error'
			: status === 'connecting' || status === 'reconnecting'
				? 'connecting'
				: status === 'disconnecting'
					? 'disconnecting'
					: hovered
						? 'wink'
						: status === 'connected'
							? 'connected'
							: 'idle',
	);
	let bridges: Animation[] = [];
	let interruptedPaths: string[] | null = null;
	let connectedCelebrated = false;
	let segmentFinished = false;
	let disposed = false;
	const contours = () => [
		...container.querySelectorAll<SVGPathElement>('svg path'),
	];
	const renderedPaths = () =>
		contours().map((path) => getComputedStyle(path).getPropertyValue('d'));

	onMount(() => {
		const media = matchMedia('(prefers-reduced-motion: reduce)');
		const updateMotion = () => {
			reducedMotion = media.matches;
		};
		const updateVisibility = () => {
			visible = !document.hidden;
		};
		updateMotion();
		updateVisibility();
		media.addEventListener('change', updateMotion);
		document.addEventListener('visibilitychange', updateVisibility);
		void Promise.all([
			import('lottie-web/build/player/lottie_light'),
			import('../../assets/lumen-expressions.json'),
		])
			.then(([{ default: lottie }, { default: expressions }]) => {
				if (disposed) return;
				for (const marker of expressions.markers)
					segments.set(marker.cm, {
						start: marker.tm,
						end: marker.tm + marker.dr,
						loop: marker.loop,
						next: 'then' in marker ? String(marker.then) : undefined,
					});
				const player = lottie.loadAnimation({
					container,
					renderer: 'svg',
					loop: true,
					autoplay: false,
					animationData: structuredClone(expressions),
					rendererSettings: {
						preserveAspectRatio: 'xMidYMid meet',
						viewBoxSize: expressions.viewport,
						progressiveLoad: false,
					},
				});
				player.addEventListener('DOMLoaded', () => {
					if (!disposed) {
						ready = true;
						animation = player;
					}
				});
				animation = player;
			})
			.catch(() => {
				/* Keep the original SVG if the lazy player cannot load. */
			});
		return () => {
			disposed = true;
			animation?.destroy();
			media.removeEventListener('change', updateMotion);
			document.removeEventListener('visibilitychange', updateVisibility);
		};
	});

	$effect(() => {
		const player = animation;
		if (!player || !ready) return;
		const name = expression;
		const connected = status === 'connected';
		if (!connected) connectedCelebrated = false;
		if (reducedMotion) {
			interruptedPaths = null;
			player.resetSegments(true);
			player.goToAndStop(0, true);
			segmentFinished = true;
			return;
		}
		let cancelled = false;
		const previous = interruptedPaths ?? renderedPaths();
		interruptedPaths = null;
		let current = segments.get(
			name === 'connected' && connectedCelebrated
				? 'connected-loop'
				: name === 'wink' && connected
					? 'wink-connected'
					: name,
		)!;
		if (name === 'connected' || (name === 'wink' && connected))
			connectedCelebrated = true;
		const play = () => {
			if (!cancelled && !disposed && !document.hidden) player.play();
		};
		const start = () => {
			segmentFinished = false;
			player.loop = current.loop;
			player.playSegments([current.start, current.end], true);
			if (document.hidden) player.pause();
		};
		const complete = () => {
			if (cancelled) return;
			const next =
				current.next ??
				(name === 'wink' ? (connected ? 'connected-loop' : 'idle') : undefined);
			if (next) {
				current = segments.get(next)!;
				start();
			} else segmentFinished = true;
		};
		player.addEventListener('complete', complete);
		start();
		player.pause();
		// Morph from the *rendered* contour when an interaction interrupts a pose.
		// The target WebView2 supports CSS SVG path interpolation; other browsers
		// gracefully use Lottie's own segment transition.
		if (CSS.supports('d', 'path("M0 0 L1 1")')) {
			bridges = contours().flatMap((path, index) => {
				const from = previous[index],
					to = getComputedStyle(path).getPropertyValue('d');
				if (!from || from === 'none' || from === to) return [];
				return [
					path.animate([{ d: from }, { d: to }], {
						duration: 180,
						easing: 'cubic-bezier(0.23, 1, 0.32, 1)',
						fill: 'both',
					}),
				];
			});
			if (document.hidden) bridges.forEach((bridge) => bridge.pause());
		}
		const pending = bridges;
		if (pending.length) {
			void Promise.allSettled(pending.map((bridge) => bridge.finished)).then(
				() => {
					if (cancelled) return;
					pending.forEach((bridge) => bridge.cancel());
					bridges = [];
					play();
				},
			);
		} else play();
		return () => {
			cancelled = true;
			interruptedPaths = renderedPaths();
			bridges.forEach((bridge) => bridge.cancel());
			bridges = [];
			// Svelte may run onMount teardown before effect teardown. Lottie clears
			// its event registry on destroy, so it must not be accessed afterwards.
			if (!disposed) player.removeEventListener('complete', complete);
		};
	});

	$effect(() => {
		const player = animation;
		if (!player || !ready) return;
		if (!visible || reducedMotion) {
			player.pause();
			bridges.forEach((bridge) => bridge.pause());
		} else if (bridges.length) bridges.forEach((bridge) => bridge.play());
		else if (!segmentFinished) player.play();
	});
</script>

<span class="lumen-face" data-expression={expression} aria-hidden="true">
	<img src={faceIcon} alt="" width="256" height="175" class:hidden={ready} />
	<div bind:this={container} class="lottie-player" class:ready></div>
</span>

<style>
	.lumen-face {
		display: block;
		position: relative;
		width: 256px;
		height: 175px;
	}
	img {
		display: block;
	}
	img.hidden {
		visibility: hidden;
	}
	.lottie-player {
		position: absolute;
		inset: 0;
		opacity: 0;
	}
	.lottie-player.ready {
		opacity: 1;
	}
	.lottie-player :global(svg) {
		overflow: visible;
	}
</style>
