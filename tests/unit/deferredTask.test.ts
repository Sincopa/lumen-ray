import { afterEach, expect, it, vi } from 'vitest';
import { createDeferredTask } from '../../src/lib/utils/deferredTask';

afterEach(() => vi.useRealTimers());

it('replaces an obsolete close callback when an interaction is reopened', () => {
	vi.useFakeTimers();
	const task = createDeferredTask();
	const close = vi.fn();
	const focus = vi.fn();
	task.schedule(close, 240);
	vi.advanceTimersByTime(100);
	task.schedule(focus, 180);
	vi.advanceTimersByTime(180);
	expect(close).not.toHaveBeenCalled();
	expect(focus).toHaveBeenCalledOnce();
	expect(vi.getTimerCount()).toBe(0);
});

it('cancels pending work on cleanup without affecting another interaction', () => {
	vi.useFakeTimers();
	const search = createDeferredTask();
	const rename = createDeferredTask();
	const searchCallback = vi.fn();
	const renameCallback = vi.fn();
	search.schedule(searchCallback, 180);
	rename.schedule(renameCallback, 180);
	search.cancel();
	search.cancel();
	vi.runAllTimers();
	expect(searchCallback).not.toHaveBeenCalled();
	expect(renameCallback).toHaveBeenCalledOnce();
});
