import { describe, expect, it } from 'vitest';
import {
	loopsBeforeGesture,
	pickGesture,
} from '../../src/lib/utils/lumenGestures';

const options = [
	{ name: 'blink', weight: 6 },
	{ name: 'glance', weight: 3 },
	{ name: 'nod', weight: 1 },
	{ name: 'hello', weight: 0 },
];

describe('pickGesture', () => {
	it('follows the weights', () => {
		// The roll is scaled by the total weight of 10.
		expect(pickGesture(options, () => 0)).toBe('blink');
		expect(pickGesture(options, () => 0.59)).toBe('blink');
		expect(pickGesture(options, () => 0.6)).toBe('glance');
		expect(pickGesture(options, () => 0.89)).toBe('glance');
		expect(pickGesture(options, () => 0.9)).toBe('nod');
		expect(pickGesture(options, () => 0.999)).toBe('nod');
	});

	it('never plays the same gesture twice in a row', () => {
		for (let roll = 0; roll < 1; roll += 0.01)
			expect(pickGesture(options, () => roll, 'blink')).not.toBe('blink');
	});

	it('never picks a gesture with no weight', () => {
		for (let roll = 0; roll < 1; roll += 0.01)
			expect(pickGesture(options, () => roll)).not.toBe('hello');
	});

	it('repeats rather than idling when there is nothing else to play', () => {
		expect(
			pickGesture([{ name: 'blink', weight: 1 }], () => 0.5, 'blink'),
		).toBe('blink');
	});

	it('has nothing to say without options', () => {
		expect(pickGesture([], () => 0.5)).toBeUndefined();
		expect(
			pickGesture([{ name: 'hello', weight: 0 }], () => 0.5),
		).toBeUndefined();
	});
});

describe('loopsBeforeGesture', () => {
	it('waits one to three breaths, favouring the short waits', () => {
		const counts = [0, 0, 0, 0];
		for (let roll = 0; roll < 1; roll += 0.001)
			counts[loopsBeforeGesture(() => roll)]++;
		expect(counts[0]).toBe(0);
		expect(counts[1]).toBeGreaterThan(counts[2]);
		expect(counts[2]).toBeGreaterThan(counts[3]);
		expect(counts[3]).toBeGreaterThan(0);
	});
});
