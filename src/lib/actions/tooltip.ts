import { shaderSettings } from '../stores/shaderSettings.svelte';
import { isAndroid } from '../platform';

/**
 * Svelte action that renders a custom tooltip above the element on hover/focus.
 * Usage: <button use:tooltip={label}>
 *
 * Hidden when shaderSettings.tooltipsEnabled is false.
 * The tail is a CSS border triangle, so it fades with the body.
 *
 * The first tooltip waits a moment so a passing pointer does not set it off.
 * Once one has been shown, its neighbours open at once and without animation,
 * so sweeping along the nav reads labels instead of replaying the entrance.
 */
const SHOW_DELAY = 300;
const WARM_WINDOW = 300;
let lastVisibleAt = 0;

function isWarm() {
	return performance.now() - lastVisibleAt < WARM_WINDOW;
}

export function tooltip(node: HTMLElement, text: string) {
	// A tap emulates hover on Android, which would leave a label over the finger.
	if (isAndroid) return { update() {}, destroy() {} };
	let tip: HTMLDivElement | null = null;
	let raf = 0;
	let timer = 0;

	function schedule() {
		if (tip || timer) return;
		if (isWarm()) {
			create(true);
			return;
		}
		timer = window.setTimeout(() => {
			timer = 0;
			create();
		}, SHOW_DELAY);
	}

	function handleFocus() {
		// Clicking focuses the button too; only keyboard focus shows the label.
		if (node.matches(':focus-visible')) create();
	}

	function create(instant = false) {
		if (tip || !text) return;
		// Respect the "show tooltips" setting
		if (!shaderSettings.tooltipsEnabled) return;

		tip = document.createElement('div');
		tip.className = 'custom-tooltip';
		tip.setAttribute('role', 'tooltip');
		tip.innerHTML = `
      <div class="custom-tooltip__body">${escapeHtml(text)}</div>
      <div class="custom-tooltip__tail"></div>
    `;

		// Inject into the closest positioned ancestor so absolute positioning works
		const parent = node.closest('.app-viewport') ?? document.body;
		parent.appendChild(tip);

		position();
		lastVisibleAt = performance.now();
		if (instant) return;

		// Grow out of the button below it (transform-origin: bottom center).
		tip.style.opacity = '0';
		tip.style.transform = 'scale(0.96)';
		// Double-RAF so the initial state is painted before the transition starts
		raf = requestAnimationFrame(() => {
			raf = requestAnimationFrame(() => {
				if (!tip) return;
				tip.style.transition =
					'opacity 150ms var(--ease-out), transform 150ms var(--ease-out)';
				tip.style.opacity = '1';
				tip.style.transform = 'scale(1)';
			});
		});
	}

	function position() {
		if (!tip) return;
		const parent = tip.parentElement!;
		const pRect = parent.getBoundingClientRect();
		const nRect = node.getBoundingClientRect();

		const tipW = tip.offsetWidth || 100;
		const tipH = tip.offsetHeight || 48;

		// Centre above the node; clamp to viewport horizontal bounds
		let left = nRect.left - pRect.left + nRect.width / 2 - tipW / 2;
		const top = nRect.top - pRect.top - tipH - 6;

		left = Math.max(8, Math.min(left, pRect.width - tipW - 8));

		tip.style.left = `${left}px`;
		tip.style.top = `${top}px`;
	}

	function destroy() {
		cancelAnimationFrame(raf);
		clearTimeout(timer);
		timer = 0;
		if (tip) {
			lastVisibleAt = performance.now();
			// Leaves faster than it came, back towards the button.
			tip.style.transition =
				'opacity 100ms var(--ease-out), transform 100ms var(--ease-out)';
			tip.style.opacity = '0';
			tip.style.transform = 'scale(0.96)';
			const el = tip;
			setTimeout(() => el.remove(), 110);
			tip = null;
		}
	}

	node.addEventListener('mouseenter', schedule);
	node.addEventListener('focus', handleFocus);
	node.addEventListener('mouseleave', destroy);
	node.addEventListener('blur', destroy);
	node.addEventListener('click', destroy);

	return {
		update(newText: string) {
			text = newText;
			if (tip) {
				const body = tip.querySelector('.custom-tooltip__body');
				if (body) body.textContent = newText;
				position();
			}
		},
		destroy() {
			destroy();
			node.removeEventListener('mouseenter', schedule);
			node.removeEventListener('focus', handleFocus);
			node.removeEventListener('mouseleave', destroy);
			node.removeEventListener('blur', destroy);
			node.removeEventListener('click', destroy);
		},
	};
}

function escapeHtml(str: string) {
	return str
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;');
}
