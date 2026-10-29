import { quintOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';
import { isAndroid } from './platform';

/*
 * Shared Svelte transitions. Curves mirror the CSS tokens in tokens.css:
 * quintOut ≈ --ease-out (cubic-bezier(0.23, 1, 0.32, 1)). Only transform and
 * opacity are animated, except `reveal`, which has to animate height.
 * Reduced motion keeps a short fade and drops all movement.
 */

export const easeOut = quintOut;

/** A per-frame blur re-rasterizes the element; phone GPUs drop frames on it. */
function blur(px: number) {
	return isAndroid ? '' : `filter: blur(${px}px);`;
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

/** Damped spring with a 5% overshoot. Same curve as --ease-morph in tokens.css. */
export function springOut(t: number): number {
	if (t >= 1) return 1;
	const damping = 0.68;
	const omega = (2 * Math.PI) / 0.42;
	const damped = omega * Math.sqrt(1 - damping * damping);
	const time = t * 0.6;
	return (
		1 -
		Math.exp(-damping * omega * time) *
			(Math.cos(damped * time) +
				((damping * omega) / damped) * Math.sin(damped * time))
	);
}

/*
 * Morph: content dissolves through a blur while its container changes shape,
 * as iOS does when a button turns into another one. `morphIn`/`morphOut`
 * cross-fade content in place; `morphWidth` also opens or closes the space.
 */
export function morphIn(
	_node: Element,
	{ duration = 420, delay = 60 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly();
	return {
		delay,
		duration,
		easing: springOut,
		css: (t, u) =>
			`opacity: ${Math.min(1, t * 1.4)}; ${blur(Math.max(0, u) * 8)}` +
			`transform: scale(${0.7 + 0.3 * t})`,
	};
}

export function morphOut(
	_node: Element,
	{ duration = 200 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly(100);
	return {
		duration,
		easing: easeOut,
		css: (t, u) =>
			`opacity: ${t}; ${blur(u * 8)} transform: scale(${1 - u * 0.3})`,
	};
}

/** A field that unrolls from its leading edge, as a label turns into an input. */
export function morphGrow(
	_node: Element,
	{ duration = 520, delay = 40 } = {},
): TransitionConfig {
	if (reduced()) return fadeOnly();
	return {
		delay,
		duration,
		easing: springOut,
		css: (t, u) =>
			`clip-path: inset(-4px ${u * 100}% -4px -4px round 999px);` +
			`opacity: ${Math.min(1, t * 2)}; ${blur(Math.max(0, u) * 6)}`,
	};
}

export function morphWidth(
	node: Element,
	{ duration = 520 } = {},
	options?: { direction?: 'in' | 'out' | 'both' },
): TransitionConfig {
	if (reduced()) return fadeOnly();
	const width = (node as HTMLElement).offsetWidth;
	// Padding shrinks too: a border-box cannot get narrower than its padding,
	// and the leftover would snap shut only when the node is removed.
	const style = getComputedStyle(node);
	const paddingLeft = parseFloat(style.paddingLeft);
	const paddingRight = parseFloat(style.paddingRight);
	// Closing does not bounce: the overshoot would reopen the gap.
	const leaving = options?.direction === 'out';
	return {
		duration: leaving ? 260 : duration,
		easing: leaving ? easeOut : springOut,
		css: (t, u) =>
			`width: ${Math.max(0, t) * width}px; padding-left: ${Math.max(0, t) * paddingLeft}px;` +
			`padding-right: ${Math.max(0, t) * paddingRight}px;` +
			`opacity: ${Math.min(1, Math.max(0, t) * 1.4)};` +
			`${blur(Math.max(0, u) * 8)} transform: scale(${0.6 + 0.4 * Math.min(1, t)})`,
	};
}

/**
 * Fade-out that leaves the layout at once, so what follows does not wait to
 * move up. The app shell has 16px padding on each side. The offset is pinned:
 * an absolute flex item without one jumps to the top of its container.
 */
export function leaveInPlace(
	node: Element,
	{ duration = 120 } = {},
): TransitionConfig {
	const { offsetTop, offsetLeft, offsetWidth } = node as HTMLElement;
	return {
		duration,
		css: (t) =>
			`position: absolute; top: ${offsetTop}px; left: ${offsetLeft}px;` +
			`width: ${offsetWidth}px; opacity: ${t}`,
	};
}

/**
 * Fade-out for a screen in an absolute stack whose container may move as it
 * leaves, for example when a header appears above it. The screen stays where
 * it was until it is gone. `top` is the container's offsetTop before the
 * change; offsets stay in CSS pixels under the UI zoom.
 */
export function leaveScreen(
	node: Element,
	{ duration = 120, top }: { duration?: number; top?: number } = {},
): TransitionConfig {
	const element = node as HTMLElement;
	const parent = element.parentElement;
	return {
		duration,
		tick: (t) => {
			element.style.opacity = String(t);
			if (top === undefined || !parent) return;
			const shift = top - parent.offsetTop;
			element.style.transform = shift ? `translateY(${shift}px)` : '';
		},
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
			`opacity: ${t}; transform: translateY(${u * y}px); ${blur(u * 2)}`,
	};
}
