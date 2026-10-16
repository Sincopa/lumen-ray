import { quintOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';

/*
 * Shared Svelte transitions. Curves mirror the CSS tokens in tokens.css:
 * quintOut ≈ --ease-out (cubic-bezier(0.23, 1, 0.32, 1)). Only transform and
 * opacity are animated, except `reveal`, which has to animate height.
 * Reduced motion keeps a short fade and drops all movement.
 */

export const easeOut = quintOut;

function reduced() {
	return (
		typeof matchMedia === 'function' &&
		matchMedia('(prefers-reduced-motion: reduce)').matches
	);
}

function fadeOnly(duration = 120): TransitionConfig {
	return { duration, css: (t) => `opacity: ${t}` };
}

/** Screen push: 1 = going deeper (enters from the right), -1 = going back. */
export function screen(
	_node: Element,
	{ dir = 1, distance = 24, duration = 260 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly(140);
	return {
		duration,
		easing: easeOut,
		css: (t, u) =>
			`opacity: ${Math.min(1, t * 1.6)}; transform: translateX(${u * dir * distance}px)`,
	};
}

/** Small lift for bars, notices and inline alerts. */
export function rise(
	_node: Element,
	{ y = 10, duration = 220, delay = 0 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly();
	return {
		delay,
		duration,
		easing: easeOut,
		css: (t, u) => `opacity: ${t}; transform: translateY(${u * y}px)`,
	};
}

/** Scale-in for menus, badges and checkmarks. Does not start at scale(0). */
export function pop(
	_node: Element,
	{ start = 0.94, duration = 180, delay = 0 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly();
	return {
		delay,
		duration,
		easing: easeOut,
		css: (t, u) => `opacity: ${t}; transform: scale(${1 - u * (1 - start)})`,
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

/** Quick cross-swap for labels and icons that change in place. */
export function swap(
	_node: Element,
	{ y = 6, duration = 180 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly(100);
	return {
		duration,
		easing: easeOut,
		css: (t, u) =>
			`opacity: ${t}; transform: translateY(${u * y}px); filter: blur(${u * 2}px)`,
	};
}
