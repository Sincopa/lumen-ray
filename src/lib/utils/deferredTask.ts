/** One pending callback per interaction; a newer request replaces the old one. */
export function createDeferredTask() {
	let timer: ReturnType<typeof setTimeout> | undefined;

	function cancel() {
		clearTimeout(timer);
		timer = undefined;
	}

	return {
		cancel,
		schedule(callback: () => void, delay: number) {
			cancel();
			timer = setTimeout(() => {
				timer = undefined;
				callback();
			}, delay);
		},
	};
}
