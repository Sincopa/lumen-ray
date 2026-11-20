import { cubicOut, quintOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';
import { isAndroid } from './platform';

/*
 * Shared Svelte transitions. Only transform, opacity and (off Android) a small
 * blur are animated, except `reveal`, which has to animate height. Svelte
 * turns these into Web Animations, so they run on the compositor and survive
 * the main thread mounting the next screen.
 *
 * Motion follows Apple's fluid-interface rules: things that move into place
 * ride a critically damped spring (no overshoot), glass that changes shape a
 * lightly underdamped one, and every enter has a matching exit that plays at
 * the same time instead of the old content vanishing first.
 *
 * Reduced motion keeps a short fade and drops all movement.
 */

export const easeOut = quintOut;

/**
 * Step response of a spring, in Apple's terms: `damping` is the damping
 * ratio (1 = critically damped, no overshoot), `response` the period in
 * seconds. `span` is how many seconds of the spring one transition covers.
 * The same curves are in tokens.css as --ease-smooth and --ease-morph.
 */
export function spring(damping: number, response: number, span: number) {
	const omega = (2 * Math.PI) / response;
	if (damping >= 1) {
		return (t: number) => {
			if (t >= 1) return 1;
			const x = omega * t * span;
			return 1 - Math.exp(-x) * (1 + x);
		};
	}
	const damped = omega * Math.sqrt(1 - damping * damping);
	return (t: number) => {
		if (t >= 1) return 1;
		const time = t * span;
		return (
			1 -
			Math.exp(-damping * omega * time) *
				(Math.cos(damped * time) +
					((damping * omega) / damped) * Math.sin(damped * time))
		);
	};
}

/** --ease-smooth: moves into place and settles, no overshoot. */
export const smooth = spring(1, 0.32, 0.42);
/** --ease-morph: glass changing shape; a 2% overshoot. */
export const springOut = spring(0.78, 0.36, 0.52);

/** A per-frame blur re-rasterizes the element; phone GPUs drop frames on it. */
function blur(px: number) {
	return isAndroid ? '' : `filter: blur(${px.toFixed(2)}px);`;
}

function reduced() {
	return (
		typeof matchMedia === 'function' &&
		matchMedia('(prefers-reduced-motion: reduce)').matches
	);
}

function fadeOnly(duration = 120): TransitionConfig {
	return { duration, css: (t) => `opacity: ${t}` };
}

type Direction = { direction?: 'in' | 'out' | 'both' };

/**
 * Entrances ride the spring; exits start at full speed (a spring from rest
 * would hesitate) and fade from the first frame.
 */
function leaving(options?: Direction) {
	return options?.direction === 'out';
}

/**
 * Keeps a leaving element where it is while the next one takes its place in
 * the flow, so both can be on screen together. Offsets are CSS pixels under
 * the UI zoom; the element's offset parent must be its containing block.
 */
function pinned(node: Element) {
	const el = node as HTMLElement;
	return (
		`position: absolute; top: ${el.offsetTop}px; left: ${el.offsetLeft}px;` +
		`width: ${el.offsetWidth}px; height: ${el.offsetHeight}px; margin: 0;`
	);
}

/**
 * Screen push, entering. 1 = going deeper (comes in from the right), -1 =
 * going back. It slides a short way and fades in early, so the content is
 * readable before it settles.
 */
export function screen(
	_node: Element,
	{ dir = 1, distance = 20, duration = 420, delay = 0 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly(160);
	return {
		delay,
		duration,
		easing: smooth,
		css: (t, u) =>
			`opacity: ${Math.min(1, t * 1.6)}; transform: translateX(${(u * dir * distance).toFixed(2)}px)`,
	};
}

/**
 * Screen push, leaving: the old screen drifts the other way and fades out
 * quickly underneath the new one. `pin` keeps an in-flow screen in place.
 */
export function screenOut(
	node: Element,
	{ dir = 1, distance = 12, duration = 200, pin = false } = {},
): TransitionConfig {
	const place = pin ? pinned(node) : '';
	if (reduced()) {
		return { duration: 120, css: (t) => `${place} opacity: ${t}` };
	}
	return {
		duration,
		easing: cubicOut,
		css: (t, u) =>
			`${place} pointer-events: none; opacity: ${t};` +
			`transform: translateX(${(-u * dir * distance).toFixed(2)}px)`,
	};
}

/** Small lift for bars, notices and inline alerts. */
export function rise(
	_node: Element,
	{ y = 10, duration = 260, delay = 0 } = {},
	options?: Direction,
): TransitionConfig {
	if (reduced()) return fadeOnly();
	const out = leaving(options);
	return {
		delay,
		duration: out ? Math.min(duration, 180) : duration,
		easing: out ? cubicOut : smooth,
		css: (t, u) =>
			`opacity: ${out ? t : Math.min(1, t * 1.4)}; transform: translateY(${(u * y).toFixed(2)}px)`,
	};
}

/** Scale-in for menus, badges and checkmarks. Does not start at scale(0). */
export function pop(
	_node: Element,
	{ start = 0.94, duration = 220, delay = 0 } = {},
	options?: Direction,
): TransitionConfig {
	if (reduced()) return fadeOnly();
	const out = leaving(options);
	return {
		delay,
		duration,
		easing: out ? cubicOut : smooth,
		css: (t, u) =>
			`opacity: ${out ? t : Math.min(1, t * 1.5)}; transform: scale(${1 - u * (1 - start)})`,
	};
}

/** Height reveal for expandable sections, with the content fading in late. */
export function reveal(
	node: Element,
	{ duration = 240 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly();
	const style = getComputedStyle(node);
	const height = parseFloat(style.height);
	const paddingTop = parseFloat(style.paddingTop);
	const paddingBottom = parseFloat(style.paddingBottom);
	const marginTop = parseFloat(style.marginTop);
	const marginBottom = parseFloat(style.marginBottom);
	return {
		duration,
		easing: easeOut,
		css: (t) =>
			`overflow: hidden; opacity: ${Math.max(0, t * 1.4 - 0.4)};` +
			`height: ${t * height}px; padding-top: ${t * paddingTop}px;` +
			`padding-bottom: ${t * paddingBottom}px; margin-top: ${t * marginTop}px;` +
			`margin-bottom: ${t * marginBottom}px;`,
	};
}

/*
 * Morph, after iOS: when a glass button turns into another one, its content
 * dissolves through a blur where it stands while the glass changes shape
 * around it, and the new content condenses out of the same blur. The content
 * barely scales; the shape does the moving. `morphIn`/`morphOut` cross-fade
 * content in place; `morphWidth` also opens or closes the space.
 */
export function morphIn(
	_node: Element,
	{ duration = 340, delay = 60 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly();
	return {
		delay,
		duration,
		easing: smooth,
		css: (t, u) =>
			`opacity: ${Math.min(1, t * 1.5)}; ${blur(u * 6)}` +
			`transform: scale(${0.94 + 0.06 * t})`,
	};
}

export function morphOut(
	_node: Element,
	{ duration = 180 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly(100);
	return {
		duration,
		easing: cubicOut,
		css: (t, u) =>
			`opacity: ${t}; ${blur(u * 6)} transform: scale(${1 - u * 0.04})`,
	};
}

/** A field that unrolls from its leading edge, as a label turns into an input. */
export function morphGrow(
	_node: Element,
	{ duration = 460, delay = 40 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly();
	return {
		delay,
		duration,
		easing: smooth,
		css: (t, u) =>
			`clip-path: inset(-4px ${(u * 100).toFixed(2)}% -4px -4px round 999px);` +
			`opacity: ${Math.min(1, t * 2)}; ${blur(u * 6)}`,
	};
}

export function morphWidth(
	node: Element,
	{ duration = 520 } = {},
	options?: Direction,
): TransitionConfig {
	if (reduced()) return fadeOnly();
	const width = (node as HTMLElement).offsetWidth;
	// Padding shrinks too: a border-box cannot get narrower than its padding,
	// and the leftover would snap shut only when the node is removed.
	const style = getComputedStyle(node);
	const paddingLeft = parseFloat(style.paddingLeft);
	const paddingRight = parseFloat(style.paddingRight);
	// Closing does not bounce: the overshoot would reopen the gap.
	const out = leaving(options);
	return {
		duration: out ? 260 : duration,
		easing: out ? cubicOut : springOut,
		css: (t, u) =>
			`width: ${Math.max(0, t) * width}px; padding-left: ${Math.max(0, t) * paddingLeft}px;` +
			`padding-right: ${Math.max(0, t) * paddingRight}px;` +
			`opacity: ${Math.min(1, Math.max(0, t) * 1.4)};` +
			`${blur(Math.max(0, u) * 6)} transform: scale(${0.7 + 0.3 * Math.min(1, t)})`,
	};
}

/** Fade-out for something that floats over the layout (no pinning needed). */
export function fadeAway(
	_node: Element,
	{ duration = 160 } = {},
): TransitionConfig {
	return {
		duration,
		easing: cubicOut,
		css: (t) => `pointer-events: none; opacity: ${t}`,
	};
}

/** Quick cross-swap for labels and icons that change in place. */
export function swap(
	_node: Element,
	{ y = 6, duration = 220 } = {},
	options?: Direction,
): TransitionConfig {
	if (reduced()) return fadeOnly(100);
	const out = leaving(options);
	return {
		duration: out ? 160 : duration,
		easing: out ? cubicOut : smooth,
		css: (t, u) =>
			`opacity: ${out ? t : Math.min(1, t * 1.4)}; transform: translateY(${(u * y).toFixed(2)}px); ${blur(u * 2)}`,
	};
}
