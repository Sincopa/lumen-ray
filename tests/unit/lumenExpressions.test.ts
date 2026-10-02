import { describe, expect, it } from 'vitest';
import expressions from '../../src/assets/lumen-expressions.json';
import { simulate } from '../../scripts/lumen-lottie/motion.mjs';
import { segments } from '../../scripts/lumen-lottie/storyboard.mjs';
import {
	ARTWORK,
	MOUTH,
	MOUTH_PIVOT,
} from '../../scripts/lumen-lottie/geometry.mjs';
import { drawn, valueAt, verify } from '../../scripts/lumen-lottie/verify.mjs';

interface Marker {
	cm: string;
	tm: number;
	dr: number;
	kind: 'loop' | 'state' | 'gesture' | 'pose';
	loop?: boolean;
	then?: string;
	mood?: 'idle' | 'connected';
	weight?: number;
}
interface Layer {
	nm: string;
	ks: { o: object };
}
interface Path {
	v: number[][];
	i: number[][];
	o: number[][];
}
const composition = expressions as unknown as {
	op: number;
	fr: number;
	w: number;
	layers: Layer[];
	markers: Marker[];
};
const markers = new Map(
	composition.markers.map((marker) => [marker.cm, marker]),
);
const marker = (name: string) => {
	const found = markers.get(name);
	if (!found) throw new Error(`No marker named ${name}`);
	return found;
};
const MARGIN = (composition.w - 256) / 2;
/** The opacity, in percent, of the soft light behind an eye at a composition frame. */
const glow = (name: string, frame: number) => {
	const layer = composition.layers.find((l) => l.nm === name);
	if (!layer) throw new Error(`No layer named ${name}`);
	const value = valueAt(layer.ks.o, frame) as number | number[];
	return Array.isArray(value) ? value[0] : value;
};

/** Every point defining a contour: vertices and both handle tips. */
const points = (path: Path) =>
	path.v.flatMap((vertex, n) => [
		vertex,
		[vertex[0] + path.i[n][0], vertex[1] + path.i[n][1]],
		[vertex[0] + path.o[n][0], vertex[1] + path.o[n][1]],
	]);
const apart = (a: Path, b: Path, offset = [0, 0]) => {
	const pa = points(a),
		pb = points(b);
	return Math.max(
		...pa.map((point, n) =>
			Math.hypot(
				point[0] - offset[0] - pb[n][0],
				point[1] - offset[1] - pb[n][1],
			),
		),
	);
};
/** How far one frame's contours are from another's, in px. */
const gap = (frameA: number, frameB: number) => {
	const a = drawn(expressions, frameA),
		b = drawn(expressions, frameB);
	return Math.max(
		apart(a.left.outline, b.left.outline),
		apart(a.right.outline, b.right.outline),
		apart(a.mouth, b.mouth),
	);
};

describe('Lumen expressions', () => {
	it('has a segment for every state the face can be in', () => {
		for (const name of [
			'idle',
			'idle:hello',
			'connecting',
			'connecting-loop',
			'connected',
			'connected-loop',
			'error',
			'error-loop',
			'disconnecting',
			'press',
			'press-connected',
			'wink',
			'wink-l',
			'wink-connected',
			'wink-connected-l',
			'pose:idle',
			'pose:connected',
			'pose:error',
		])
			expect(markers.has(name), name).toBe(true);
	});

	it('lays segments end to end inside the composition, leaving a gap frame', () => {
		let cursor = 0;
		for (const { cm, tm, dr } of composition.markers) {
			expect(tm, cm).toBe(cursor);
			cursor = tm + dr + 2;
		}
		expect(cursor - 1).toBe(composition.op);
	});

	it('chains every follow-up segment to one that exists', () => {
		for (const { cm, then } of composition.markers)
			if (then) expect(markers.has(then), `${cm} → ${then}`).toBe(true);
	});

	it('lets both moods fidget with weighted gestures that come back to rest', () => {
		for (const mood of ['idle', 'connected'] as const) {
			const gestures = composition.markers.filter(
				(m) => m.kind === 'gesture' && m.mood === mood && (m.weight ?? 0) > 0,
			);
			expect(gestures.length, mood).toBeGreaterThanOrEqual(5);
			const rest = marker(mood === 'idle' ? 'idle' : 'connected-loop');
			expect(rest.loop).toBe(true);
			expect(rest.mood).toBe(mood);
			for (const { cm, tm, dr } of gestures)
				expect(gap(tm + dr, rest.tm), cm).toBeLessThan(0.4);
		}
	});

	it('closes every loop, so it can run forever without a pop', () => {
		for (const { cm, tm, dr, loop } of composition.markers)
			if (loop) expect(gap(tm, tm + dr), cm).toBeLessThan(0.02);
	});

	it('hands each state over to its loop where they meet', () => {
		for (const { cm, tm, dr, then } of composition.markers)
			if (then)
				expect(gap(tm + dr, marker(then).tm), `${cm} → ${then}`).toBeLessThan(
					0.4,
				);
		// A wink or a disconnect returns to the mood's resting loop.
		expect(
			gap(marker('wink').tm + marker('wink').dr, marker('idle').tm),
		).toBeLessThan(0.4);
		expect(
			gap(marker('wink-l').tm + marker('wink-l').dr, marker('idle').tm),
		).toBeLessThan(0.4);
		for (const name of ['wink-connected', 'wink-connected-l'])
			expect(
				gap(marker(name).tm + marker(name).dr, marker('connected-loop').tm),
				name,
			).toBeLessThan(0.4);
	});

	it('holds the last frame of a one-shot, because a player stops one frame early', () => {
		for (const { cm, tm, dr, loop } of composition.markers)
			if (!loop && dr > 2)
				expect(gap(tm + dr - 1, tm + dr), cm).toBeLessThan(0.25);
	});

	it('lights up behind the eyes only while connected', () => {
		for (const name of ['Left glow', 'Right glow']) {
			expect(glow(name, marker('idle').tm), name).toBe(0);
			expect(glow(name, marker('connecting-loop').tm), name).toBe(0);
			expect(glow(name, marker('error-loop').tm), name).toBe(0);
			expect(glow(name, marker('connected-loop').tm)).toBeGreaterThan(60);
			// ...with a flash as it comes on, and dark again once it has powered down.
			expect(glow(name, marker('connected').tm + 24)).toBeGreaterThan(80);
			expect(
				glow(name, marker('disconnecting').tm + marker('disconnecting').dr),
				name,
			).toBe(0);
		}
	});

	it('rests as the supplied artwork', () => {
		const rest = drawn(expressions, marker('idle').tm);
		expect(
			apart(rest.left.outline, ARTWORK.left, [MARGIN, MARGIN]),
		).toBeLessThan(0.03);
		expect(
			apart(rest.right.outline, ARTWORK.right, [MARGIN, MARGIN]),
		).toBeLessThan(0.03);
		// The smile is a round-capped stroke along the artwork's arc.
		const [cx, cy] = MOUTH_PIVOT;
		const radius = 1 / MOUTH.curvature;
		for (const [x, y] of rest.mouth.v)
			expect(
				Math.hypot(x - MARGIN - cx, y - MARGIN - cy),
				'smile radius',
			).toBeCloseTo(radius, 1);
		expect(rest.mouth.v[0][0] - MARGIN).toBeCloseTo(
			cx - radius * Math.sin(0.748),
			0,
		);
		expect(rest.mouth.v[4][0] - MARGIN).toBeCloseTo(
			cx + radius * Math.sin(0.748),
			0,
		);
	});

	it('stays faithful to the storyboard it was generated from', () => {
		// If this fails the asset is stale: run `npm run generate:lottie`.
		const baked = segments().map(({ timeline, ...segment }) => {
			const { frames } = simulate(timeline);
			if (!segment.loop) frames.push(frames.at(-1));
			return { ...segment, frames };
		});
		expect(baked.map((segment) => segment.name)).toEqual(
			composition.markers.map((m) => m.cm),
		);
		const report = verify(expressions, baked);
		for (const [name, { worst }] of Object.entries(report))
			expect(worst, name).toBeLessThan(1);
	}, 60_000);

	it('stays small enough to ship', () => {
		expect(JSON.stringify(expressions).length).toBeLessThan(600 * 1024);
	});
});
