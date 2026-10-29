/**
 * Runs all requestAnimationFrame callbacks together, on every other display
 * frame.
 *
 * Every frame a page asks for costs an Android WebView a whole main-frame
 * lifecycle (style, layout, paint, commit) plus a draw on the app's UI thread,
 * even when nothing changed. On a Kirin 710 the face animation's 60 requests a
 * second kept two cores busy on an idle screen. Callbacks share one queue, so
 * the face and the background shader draw on the same frames; after an idle
 * period the first request still runs on the next frame. CSS transitions are
 * not affected.
 */
export function paceAnimationFrames(fps = 30) {
	const native = window.requestAnimationFrame.bind(window);
	// Requested this long after a frame, the native callback lands one display
	// frame (60 Hz) later.
	const defer = 1000 / fps - 1000 / 60 / 2;
	let queue = new Map<number, FrameRequestCallback>();
	let nextId = 1;
	let scheduled = false;
	let lastFrame = -Infinity;

	const flush = (time: number) => {
		scheduled = false;
		lastFrame = performance.now();
		const callbacks = queue;
		queue = new Map();
		for (const callback of callbacks.values()) callback(time);
	};

	const schedule = () => {
		if (scheduled) return;
		scheduled = true;
		const since = performance.now() - lastFrame;
		if (since < defer) window.setTimeout(() => native(flush), defer - since);
		else native(flush);
	};

	window.requestAnimationFrame = (callback) => {
		const id = nextId++;
		queue.set(id, callback);
		schedule();
		return id;
	};

	window.cancelAnimationFrame = (id) => {
		queue.delete(id);
	};
}
