import { describe, expect, it } from 'vitest';
import { bridgeDuration, displacement } from '../../src/lib/utils/lumenBridge';

describe('displacement', () => {
	it('is zero for identical renderings', () => {
		const matrix = 'matrix(1, 0, 0, 1, 32, 32)';
		expect(displacement('transform', matrix, matrix)).toBe(0);
		const d = 'path("M 1.5 2.5 C 3 4, 5 6, 7 8")';
		expect(displacement('d', d, d)).toBe(0);
	});

	it('measures a translation of the face in px', () => {
		expect(
			displacement(
				'transform',
				'matrix(1, 0, 0, 1, 32, 32)',
				'matrix(1, 0, 0, 1, 35, 36)',
			),
		).toBeCloseTo(5, 6);
	});

	it('weighs rotation and scale by how far they carry the corners of the face', () => {
		const turned = 'matrix(0.99985, 0.01745, -0.01745, 0.99985, 0, 0)'; // 1°
		const gap = displacement('transform', 'matrix(1, 0, 0, 1, 0, 0)', turned);
		// A degree swings a corner ~300 px from the origin by ~5 px.
		expect(gap).toBeGreaterThan(3);
		expect(gap).toBeLessThan(8);
	});

	it('measures the largest move of any contour point', () => {
		expect(
			displacement(
				'd',
				'path("M 0 0 C 1 1, 2 2, 3 3")',
				'path("M 0 0 C 1 1, 2 2, 3 5.5")',
			),
		).toBeCloseTo(2.5, 6);
	});

	it('treats a light fading fully on or off as a long way', () => {
		expect(displacement('opacity', '1', '0')).toBe(60);
		expect(displacement('opacity', '0.5', '0.5')).toBe(0);
	});

	it('reports contours that cannot be morphed as infinitely far', () => {
		expect(
			displacement('d', 'path("M 0 0 L 1 1")', 'path("M 0 0 C 1 1, 2 2, 3 3")'),
		).toBe(Infinity);
		expect(displacement('transform', 'none', 'matrix(1, 0, 0, 1, 0, 0)')).toBe(
			Infinity,
		);
	});
});

describe('bridgeDuration', () => {
	it('cuts over differences of a pixel or so, so a reaction starts at once', () => {
		expect(bridgeDuration(0, 180)).toBe(0);
		expect(bridgeDuration(1.4, 180)).toBe(0);
	});

	it('takes longer the further the face has to travel, up to the limit', () => {
		const short = bridgeDuration(2, 180);
		const medium = bridgeDuration(8, 180);
		expect(short).toBeGreaterThan(0);
		expect(medium).toBeGreaterThan(short);
		expect(bridgeDuration(500, 180)).toBe(180);
		expect(bridgeDuration(500, 90)).toBe(90);
	});
});
