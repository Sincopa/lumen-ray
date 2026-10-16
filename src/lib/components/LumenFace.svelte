<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { AnimationItem } from 'lottie-web';
	import faceIcon from '../../assets/lumen-face.svg';
	import { bridgeDuration, displacement } from '../utils/lumenBridge';
	import { loopsBeforeGesture, pickGesture } from '../utils/lumenGestures';
	import type { VpnConnectionState } from '../types/models';

	let {
		status = 'disconnected',
		hovered = false,
		pressed = false,
		error = false,
	}: {
		status?: VpnConnectionState;
		hovered?: boolean;
		pressed?: boolean;
		error?: boolean;
	} = $props();
	let container: HTMLDivElement;
	let animation = $state.raw<AnimationItem | null>(null);
	let ready = $state(false);
	let reducedMotion = $state(false);
	let visible = $state(true);
	let segmentName = $state('');
	type Expression =
		| 'idle'
		| 'connecting'
		| 'connected'
		| 'press'
		| 'wink'
		| 'error'
		| 'disconnecting';
	type Mood = 'idle' | 'connected';
	interface Segment {
		name: string;
		start: number;
		end: number;
		loop: boolean;
		kind: 'loop' | 'state' | 'gesture' | 'pose';
		next?: string;
		mood?: Mood;
		weight: number;
		/** How long, in ms, the face takes to move from wherever it was to this segment's first pose. */
		bridge: number;
	}
	interface Marker {
		cm: string;
		tm: number;
		dr: number;
		kind: Segment['kind'];
		loop?: boolean;
		then?: string;
		mood?: Mood;
		weight?: number;
		bridge?: number;
	}
	const segments = new SvelteMap<string, Segment>();
	const gestures: Record<Mood, { name: string; weight: number }[]> = {
		idle: [],
		connected: [],
	};
	let expression: Expression = $derived(
		error
			? 'error'
			: status === 'connecting' || status === 'reconnecting'
				? 'connecting'
				: status === 'disconnecting'
					? 'disconnecting'
					: pressed
						? 'press'
						: hovered
							? 'wink'
							: status === 'connected'
								? 'connected'
								: 'idle',
	);

	// What the face looks like right now: where each layer sits and what each contour is.
	interface Look {
		transforms: string[];
		opacities: string[];
		contours: string[];
	}
	const layerElements = () => [
		...container.querySelectorAll<SVGGElement>('svg > g > g'),
	];
	const contourElements = () => [
		...container.querySelectorAll<SVGPathElement>('svg path'),
	];
	const look = (): Look => ({
		transforms: layerElements().map(
			(layer) => getComputedStyle(layer).transform,
		),
		opacities: layerElements().map((layer) => getComputedStyle(layer).opacity),
		contours: contourElements().map((path) =>
			getComputedStyle(path).getPropertyValue('d'),
		),
	});

	let bridges: Animation[] = [];
	let interrupted: Look | null = null;
	let connectedCelebrated = false;
	let greeted = false;
	let lastGesture: string | undefined;
	let segmentFinished = false;
	let disposed = false;
	let playing: Segment | undefined;
	let lastExpression: Expression | undefined;

	/**
	 * Carries the face from the look it had to the first pose of the next segment, so an
	 * interaction never snaps. Lottie draws plain SVG, so the layer transforms and the
	 * contours can be interpolated by the browser; the target WebView2 can morph path
	 * data, and elsewhere the face simply cuts to the new segment. Differences too small
	 * to see are skipped, so a reaction starts at once instead of after a pointless blend.
	 */
	function bridge(from: Look, limit: number): Animation[] {
		const pairs: [Element, 'transform' | 'opacity' | 'd', string, string][] =
			[];
		layerElements().forEach((layer, index) => {
			const style = getComputedStyle(layer);
			pairs.push(
				[layer, 'transform', from.transforms[index], style.transform],
				[layer, 'opacity', from.opacities[index], style.opacity],
			);
		});
		if (CSS.supports('d', 'path("M0 0 L1 1")'))
			contourElements().forEach((path, index) =>
				pairs.push([
					path,
					'd',
					from.contours[index],
					getComputedStyle(path).getPropertyValue('d'),
				]),
			);
		const movers = pairs
			.map(([element, property, a, b]) => ({
				element,
				property,
				a,
				b,
				gap:
					a && b && a !== 'none' && b !== 'none'
						? displacement(property, a, b)
						: Infinity,
			}))
			.filter(({ gap }) => Number.isFinite(gap) && gap > 0.01);
		const duration = bridgeDuration(
			Math.max(0, ...movers.map(({ gap }) => gap)),
			limit,
		);
		if (!duration) return [];
		return movers.map(({ element, property, a, b }) =>
			element.animate([{ [property]: a }, { [property]: b }], {
				duration,
				easing: 'cubic-bezier(0.23, 1, 0.32, 1)',
				fill: 'both',
			}),
		);
	}

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
				for (const marker of expressions.markers as Marker[]) {
					segments.set(marker.cm, {
						name: marker.cm,
						start: marker.tm,
						end: marker.tm + marker.dr,
						loop: marker.loop ?? false,
						kind: marker.kind,
						next: marker.then,
						mood: marker.mood,
						weight: marker.weight ?? 0,
						bridge: marker.bridge ?? 180,
					});
					if (marker.kind === 'gesture' && marker.mood && marker.weight)
						gestures[marker.mood].push({
							name: marker.cm,
							weight: marker.weight,
						});
				}
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
			// Reduced motion keeps the mood but drops the movement: a still pose per state.
			interrupted = null;
			const pose = segments.get(
				name === 'error'
					? 'pose:error'
					: name === 'connected' || (name === 'wink' && connected)
						? 'pose:connected'
						: 'pose:idle',
			)!;
			player.resetSegments(true);
			player.goToAndStop(pose.start, true);
			segmentName = pose.name;
			segmentFinished = true;
			playing = undefined;
			return;
		}
		const before = lastExpression;
		lastExpression = name;
		// Disconnecting plays out as an "oh", a slump, a nap, then a wake-up.
		// The status can move on to "disconnected" a moment after it starts, but once
		// begun the sequence finishes unless the user does something else.
		const finishing =
			before === 'disconnecting' &&
			(name === 'idle' || name === 'wink') &&
			playing?.name === 'disconnecting' &&
			!segmentFinished;
		let cancelled = false;
		const previous = interrupted ?? look();
		interrupted = null;
		const baseOf = (mood: Mood | undefined) =>
			segments.get(mood === 'connected' ? 'connected-loop' : 'idle')!;
		let current!: Segment;
		let loopsLeft = 0;
		const play = (segment: Segment, hold = false) => {
			current = segment;
			playing = segment;
			segmentName = segment.name;
			segmentFinished = false;
			// A resting loop lets the face fidget now and then: after a few breaths it
			// plays one gesture, then goes back to breathing.
			loopsLeft =
				segment.kind === 'loop' && segment.mood
					? loopsBeforeGesture(Math.random)
					: 0;
			player.loop = segment.loop;
			player.playSegments([segment.start, segment.end], true);
			if (hold || document.hidden) player.pause();
		};
		const complete = () => {
			if (cancelled) return;
			const next =
				current.next ??
				(current.kind === 'gesture'
					? baseOf(current.mood).name
					: name === 'wink'
						? baseOf(connected ? 'connected' : 'idle').name
						: undefined);
			if (next) play(segments.get(next)!);
			else segmentFinished = true;
		};
		const loopComplete = () => {
			if (cancelled || current.kind !== 'loop' || !current.mood) return;
			if (--loopsLeft > 0) return;
			const choice = pickGesture(
				gestures[current.mood],
				Math.random,
				lastGesture,
			);
			if (!choice) return;
			lastGesture = choice;
			play(segments.get(choice)!);
		};
		const wink = `wink${connected ? '-connected' : ''}${Math.random() < 0.4 ? '-l' : ''}`;
		const entry =
			name === 'wink'
				? wink
				: name === 'press'
					? `press${connected ? '-connected' : ''}`
					: name === 'connected' && connectedCelebrated
						? 'connected-loop'
						: name === 'idle' && !greeted
							? 'idle:hello'
							: name;
		if (name === 'connected' || (name === 'wink' && connected))
			connectedCelebrated = true;
		if (name === 'idle') greeted = true;
		player.addEventListener('complete', complete);
		player.addEventListener('loopComplete', loopComplete);
		const resume = () => {
			if (!cancelled && !disposed && !document.hidden) player.play();
		};
		if (finishing) {
			current = playing!;
			resume();
		} else {
			play(segments.get(entry)!, true);
			bridges = bridge(previous, current.bridge);
			if (document.hidden) bridges.forEach((item) => item.pause());
			const pending = bridges;
			if (pending.length) {
				void Promise.allSettled(pending.map((item) => item.finished)).then(
					() => {
						if (cancelled) return;
						pending.forEach((item) => item.cancel());
						bridges = [];
						resume();
					},
				);
			} else resume();
		}
		return () => {
			cancelled = true;
			interrupted = look();
			bridges.forEach((item) => item.cancel());
			bridges = [];
			// Svelte may run onMount teardown before effect teardown. Lottie clears
			// its event registry on destroy, so it must not be accessed afterwards.
			if (!disposed) {
				player.removeEventListener('complete', complete);
				player.removeEventListener('loopComplete', loopComplete);
			}
		};
	});

	$effect(() => {
		const player = animation;
		if (!player || !ready) return;
		if (!visible || reducedMotion) {
			player.pause();
			bridges.forEach((item) => item.pause());
		} else if (bridges.length) bridges.forEach((item) => item.play());
		else if (!segmentFinished) player.play();
	});
</script>

<span
	class="lumen-face"
	data-expression={expression}
	data-segment={segmentName || undefined}
	aria-hidden="true"
>
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
