/**
 * Lumen's macOS window did not scroll its lists with the trackpad or a mouse
 * wheel, while the same page scrolls in WebKit elsewhere: the native scroll
 * path of WKWebView inside the Tauri window lost the gesture. On macOS the app
 * moves the list itself. A window-level wheel listener takes the gesture, finds
 * the nearest container under the pointer that can still move that way, and
 * scrolls it. Trackpad momentum arrives as further wheel events and keeps going.
 */

const SCROLLS = /^(auto|scroll|overlay)$/;

type Axis = 'x' | 'y';

/** `el` scrolls on `axis` and has room left in the direction of `delta`. */
export function canScroll(el: Element, axis: Axis, delta: number): boolean {
	const style = getComputedStyle(el);
	if (!SCROLLS.test(axis === 'y' ? style.overflowY : style.overflowX)) {
		return false;
	}
	const position = axis === 'y' ? el.scrollTop : el.scrollLeft;
	const room =
		axis === 'y'
			? el.scrollHeight - el.clientHeight
			: el.scrollWidth - el.clientWidth;
	if (room <= 0) return false;
	return delta > 0 ? position < room - 0.5 : position > 0.5;
}

/** The innermost container from `start` outwards that can move by `delta`. */
export function scrollerFor(
	start: EventTarget | null,
	axis: Axis,
	delta: number,
): Element | null {
	let el = start instanceof Element ? start : null;
	while (el && el !== document.documentElement) {
		if (canScroll(el, axis, delta)) return el;
		el = el.parentElement;
	}
	return null;
}

function pixels(event: WheelEvent, delta: number) {
	if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) return delta * 16;
	if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) {
		return delta * window.innerHeight;
	}
	return delta;
}

/** Scrolls the containers under the pointer; returns the uninstaller. */
export function installWheelScroll(): () => void {
	const onWheel = (event: WheelEvent) => {
		// Pinch zoom, and controls that handle the wheel themselves.
		if (event.defaultPrevented || event.ctrlKey) return;
		const dy = pixels(event, event.deltaY);
		const dx = pixels(event, event.deltaX);
		let moved = false;
		const vertical = dy !== 0 && scrollerFor(event.target, 'y', dy);
		if (vertical) {
			vertical.scrollTop += dy;
			moved = true;
		}
		const horizontal = dx !== 0 && scrollerFor(event.target, 'x', dx);
		if (horizontal) {
			horizontal.scrollLeft += dx;
			moved = true;
		}
		// Only a gesture Lumen carried out: the rest keeps its default.
		if (moved) event.preventDefault();
	};
	window.addEventListener('wheel', onWheel, { passive: false });
	return () => window.removeEventListener('wheel', onWheel);
}
