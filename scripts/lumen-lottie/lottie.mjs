import { MOUTH, layersOf } from './geometry.mjs';

const roundTo = (decimals) => {
	const scale = 10 ** decimals;
	return (value) => {
		const rounded = Math.round(value * scale) / scale;
		return rounded === 0 ? 0 : rounded;
	};
};
const round2 = roundTo(2);
const round3 = roundTo(3);

// ---------------------------------------------------------------------------
// Keyframe fitting.
//
// Motion is simulated at 60 fps, but shipping every frame would be huge. Each run of
// frames is described by two keyframes plus a cubic-Bézier easing — the same
// ease-in / ease-out / overshoot a motion designer would hand-key — and the number
// of keyframes is minimised while the reconstruction stays within a tolerance.
// ---------------------------------------------------------------------------

const bx = (s, a, b) => 3 * (1 - s) * (1 - s) * s * a + 3 * (1 - s) * s * s * b + s * s * s;

/** Easing exactly as lottie-web evaluates it (bezier-easing): progress for time u. */
export function easingFunction([x1, y1, x2, y2]) {
	if (x1 === y1 && x2 === y2) return (u) => u;
	return (u) => {
		if (u <= 0) return 0;
		if (u >= 1) return 1;
		let lo = 0,
			hi = 1;
		for (let n = 0; n < 40; n++) {
			const mid = (lo + hi) / 2;
			if (bx(mid, x1, x2) < u) lo = mid;
			else hi = mid;
		}
		return bx((lo + hi) / 2, y1, y2);
	};
}

const LINEAR = [0, 0, 1, 1];
const X_GRID = [0.08, 0.2, 0.33, 0.5, 0.67, 0.82, 0.94];

/** Least-squares y-handles for fixed x-handles; null if degenerate. */
function solveHandles(x1, x2, us, ps) {
	let a11 = 0,
		a12 = 0,
		a22 = 0,
		b1 = 0,
		b2 = 0;
	for (let n = 0; n < us.length; n++) {
		let lo = 0,
			hi = 1;
		for (let k = 0; k < 28; k++) {
			const mid = (lo + hi) / 2;
			if (bx(mid, x1, x2) < us[n]) lo = mid;
			else hi = mid;
		}
		const s = (lo + hi) / 2;
		const f1 = 3 * (1 - s) * (1 - s) * s,
			f2 = 3 * (1 - s) * s * s,
			q = ps[n] - s * s * s;
		a11 += f1 * f1;
		a12 += f1 * f2;
		a22 += f2 * f2;
		b1 += f1 * q;
		b2 += f2 * q;
	}
	const det = a11 * a22 - a12 * a12;
	if (Math.abs(det) < 1e-12) return null;
	return [(b1 * a22 - b2 * a12) / det, (a11 * b2 - a12 * b1) / det];
}

/**
 * Tries to express frames a..b as `start + (end − start)·ease(u)`. Returns the
 * easing, or null when no single ease keeps every in-between frame within its
 * allowance (`allow[n][k]`, in the property's own units; `null` = unrendered frame).
 */
function fitInterval(frames, rounded, a, b, allow) {
	const size = frames[a].length;
	const start = rounded[a],
		end = rounded[b];
	const delta = new Float64Array(size);
	let dd = 0;
	for (let k = 0; k < size; k++) {
		delta[k] = end[k] - start[k];
		dd += delta[k] * delta[k];
	}
	const length = b - a;
	if (dd < 1e-12) {
		for (let n = a + 1; n < b; n++)
			if (allow[n])
				for (let k = 0; k < size; k++)
					if (Math.abs(frames[n][k] - start[k]) > allow[n][k]) return null;
		return LINEAR;
	}
	const us = [],
		ps = [],
		at = [],
		residuals = [];
	for (let n = a + 1; n < b; n++) {
		if (!allow[n]) continue;
		const frame = frames[n];
		let dot = 0;
		for (let k = 0; k < size; k++) dot += (frame[k] - start[k]) * delta[k];
		const p = dot / dd;
		const r = new Float64Array(size);
		for (let k = 0; k < size; k++) {
			r[k] = frame[k] - start[k] - delta[k] * p;
			if (Math.abs(r[k]) > allow[n][k]) return null;
		}
		us.push((n - a) / length);
		ps.push(p);
		at.push(n);
		residuals.push(r);
	}
	const count = us.length;
	const error = (ease) => {
		const f = easingFunction(ease);
		let worst = 0;
		for (let n = 0; n < count; n++) {
			const gap = ps[n] - f(us[n]);
			for (let k = 0; k < size; k++)
				worst = Math.max(worst, Math.abs(residuals[n][k] + delta[k] * gap) / allow[at[n]][k]);
		}
		return worst;
	};
	if (error(LINEAR) <= 1) return LINEAR;
	let best = null;
	const consider = (x1, x2) => {
		const y = solveHandles(x1, x2, us, ps);
		if (!y) return;
		const ease = [round3(x1), round3(y[0]), round3(x2), round3(y[1])];
		const e = error(ease);
		if (!best || e < best.error) best = { error: e, ease };
	};
	for (const x1 of X_GRID) for (const x2 of X_GRID) consider(x1, x2);
	if (!best) return null;
	const [bx1, , bx2] = best.ease;
	for (const dx1 of [-0.06, -0.03, 0, 0.03, 0.06])
		for (const dx2 of [-0.06, -0.03, 0, 0.03, 0.06])
			consider(
				Math.min(0.99, Math.max(0.01, bx1 + dx1)),
				Math.min(0.99, Math.max(0.01, bx2 + dx2)),
			);
	return best.error <= 1 ? best.ease : null;
}

/**
 * Errors matter less while something moves fast, so each frame's allowance is the
 * base tolerance plus a share of how far that value travels per frame. Stillness
 * (where error would show as jitter) keeps the strict base tolerance.
 */
const SPEED_SHARE = 0.05;

function allowances(track, base) {
	return track.map((frame, n) =>
		frame.map((_, k) => {
			const before = track[Math.max(0, n - 1)][k],
				after = track[Math.min(track.length - 1, n + 1)][k];
			return base[k] + (SPEED_SHARE * Math.abs(after - before)) / 2;
		}),
	);
}

/**
 * Greedy longest-interval keyframe selection with galloping + bisection. Frames listed
 * in `must` always become keyframes (the ends of a loop, which must meet exactly).
 */
export function fitTrack(frames, allow, round, must = []) {
	const rounded = frames.map((frame) => frame.map(round));
	const last = frames.length - 1;
	const keys = [{ index: 0, ease: LINEAR }];
	let a = 0;
	while (a < last) {
		const reach = Math.min(last, ...must.filter((index) => index > a));
		let good = a + 1,
			goodEase = LINEAR,
			bad = null,
			step = 2;
		for (;;) {
			const b = Math.min(reach, a + step);
			const ease = fitInterval(frames, rounded, a, b, allow);
			if (ease) {
				good = b;
				goodEase = ease;
				if (b === reach) break;
				step *= 2;
			} else {
				bad = b;
				break;
			}
		}
		if (bad !== null) {
			let lo = good,
				hi = bad;
			while (hi - lo > 1) {
				const mid = (lo + hi) >> 1;
				const ease = fitInterval(frames, rounded, a, mid, allow);
				if (ease) {
					lo = mid;
					goodEase = ease;
				} else hi = mid;
			}
			good = lo;
		}
		keys.at(-1).ease = goodEase;
		keys.push({ index: good });
		a = good;
	}
	return { keys, rounded, allow };
}

/** Largest reconstruction error, in units of the allowance (≤ 1 means within budget). */
export function reconstructionError(frames, { keys, rounded, allow }) {
	let worst = 0;
	for (let n = 0; n < keys.length - 1; n++) {
		const a = keys[n].index,
			b = keys[n + 1].index;
		const f = easingFunction(keys[n].ease);
		for (let m = a; m <= b; m++) {
			if (!allow[m]) continue;
			const p = f((m - a) / (b - a));
			for (let k = 0; k < frames[m].length; k++)
				worst = Math.max(
					worst,
					Math.abs(frames[m][k] - (rounded[a][k] + (rounded[b][k] - rounded[a][k]) * p)) /
						allow[m][k],
				);
		}
	}
	return worst;
}

export const stats = { keyframes: 0, frames: 0, worst: 0, tracks: 0, animated: 0, byTrack: [] };

const flatten = (path) => [...path.v.flat(), ...path.i.flat(), ...path.o.flat()];

function unflatten(flat, closed, count) {
	const take = (offset) =>
		Array.from({ length: count }, (_, n) => [flat[offset + n * 2], flat[offset + n * 2 + 1]]);
	return { c: closed, v: take(0), i: take(count * 2), o: take(count * 4) };
}

/**
 * One animated Lottie property. `read(frame)` yields the property's numbers for a
 * baked frame. All segments share one timeline (separated by an unrendered frame),
 * so stillness that spans several segments costs nothing; a property that never
 * changes is stored without keyframes.
 */
function property(
	segments,
	read,
	{ kind = 'value', tolerance, decimals = 3, closed, count, label = '' },
) {
	const round = decimals === 2 ? round2 : round3;
	const tracks = segments.map((segment) => segment.frames.map(read));
	const first = tracks[0][0];
	const constant = tracks.every((track) =>
		track.every((frame) => frame.every((value, k) => Math.abs(value - first[k]) < 1e-9)),
	);
	stats.tracks++;
	if (constant) {
		const value = first.map(round);
		return kind === 'path'
			? { a: 0, k: unflatten(value, closed, count) }
			: { a: 0, k: value.length === 1 ? value[0] : value };
	}
	stats.animated++;
	const frames = [],
		allow = [],
		must = [];
	tracks.forEach((track, index) => {
		if (segments[index].loop) must.push(frames.length, frames.length + track.length - 1);
		frames.push(...track);
		allow.push(...allowances(track, tolerance));
		if (index < tracks.length - 1) {
			frames.push(tracks[index + 1][0]);
			allow.push(null); // the unused frame between two segments
		}
	});
	const fit = fitTrack(frames, allow, round, must);
	stats.worst = Math.max(stats.worst, reconstructionError(frames, fit));
	stats.frames += frames.length;
	stats.keyframes += fit.keys.length;
	stats.byTrack.push([label, fit.keys.length]);
	const keys = fit.keys.map((key) => {
		const [x1, y1, x2, y2] = key.ease ?? LINEAR;
		const value = fit.rounded[key.index];
		return {
			t: key.index,
			s: kind === 'path' ? [unflatten(value, closed, count)] : value,
			// Paths take scalar easing; other properties are cached per dimension.
			o: kind === 'path' ? { x: x1, y: y1 } : { x: [x1], y: [y1] },
			i: kind === 'path' ? { x: x2, y: y2 } : { x: [x2], y: [y2] },
		};
	});
	delete keys.at(-1).o;
	delete keys.at(-1).i;
	return { a: 1, k: keys };
}

const INK = [0, 0, 0, 1];
/** The soft light that comes on behind the eyes when Lumen is connected. */
const HALO = { radius: 92, y: 64, peak: 0.34, shoulder: 0.15 };
const INK_OPACITY = 67;
/** Tolerances share one visual budget (px at the face's edge, ~140 px from its pivot). */
const REACH = 140;

/**
 * Turns baked segments into a single Lottie composition. `segments` is a list of
 * { name, frames: channelMap[], loop?, then?, kind?, mood?, weight? }.
 */
export function assemble(segments, { tolerance = 0.03, margin = 64 } = {}) {
	Object.assign(stats, { keyframes: 0, frames: 0, worst: 0, tracks: 0, animated: 0, byTrack: [] });
	const rendered = segments.map((segment) => ({
		...segment,
		frames: segment.frames.map(layersOf),
	}));
	let total = 0;
	const markers = [];
	for (const segment of rendered) {
		const duration = segment.frames.length - 1;
		const { frames: _frames, name, ...meta } = segment;
		markers.push({ cm: name, tm: total, dr: duration, ...meta });
		total += duration + 2;
	}
	total -= 1;

	const px = [tolerance];
	const degrees = [(tolerance * 180) / Math.PI / REACH];
	const percent = [(tolerance / REACH) * 100, (tolerance / REACH) * 100, 1];
	const transform = (pick, name, [offsetX, offsetY] = [0, 0]) => {
		const anchor = pick(rendered[0].frames[0]).anchor;
		return {
			o: { a: 0, k: 100 },
			a: { a: 0, k: [anchor[0], anchor[1], 0] },
			p: {
				s: true,
				x: property(rendered, (f) => [pick(f).anchor[0] + pick(f).x + offsetX], {
					tolerance: px,
					label: name + '.x',
				}),
				y: property(rendered, (f) => [pick(f).anchor[1] + pick(f).y + offsetY], {
					tolerance: px,
					label: name + '.y',
				}),
			},
			r: property(rendered, (f) => [pick(f).rot], { tolerance: degrees, label: name + '.rot' }),
			s: property(rendered, (f) => [pick(f).sx * 100, pick(f).sy * 100, 100], {
				tolerance: percent,
				label: name + '.scale',
			}),
		};
	};
	const eyeTolerance = Array.from({ length: 24 }, () => tolerance);
	const mouthTolerance = Array.from({ length: 30 }, () => tolerance);
	const contour = (pick, label, closed, count, tol) => ({
		ty: 'sh',
		ks: property(rendered, (f) => flatten(pick(f)), {
			kind: 'path',
			tolerance: tol,
			decimals: 2,
			closed,
			count,
			label,
		}),
	});
	const eyeShapes = (side) => [
		{ ...contour((f) => f[side].outline, side + '.eye', true, 4, eyeTolerance), nm: 'Eye' },
		{ ...contour((f) => f[side].glint, side + '.glint', true, 4, eyeTolerance), nm: 'Glint' },
		{ ty: 'fl', nm: 'Ink', c: { a: 0, k: INK }, o: { a: 0, k: INK_OPACITY }, r: 2 },
	];
	const layer = (ind, nm, ty, extra) => ({
		ddd: 0,
		ind,
		ty,
		nm,
		...extra,
		ip: 0,
		op: total,
		st: 0,
		bm: 0,
	});
	const layers = [
		layer(1, 'Left eye', 4, {
			parent: 4,
			ks: transform((f) => f.left, 'L'),
			shapes: eyeShapes('left'),
		}),
		layer(2, 'Right eye', 4, {
			parent: 4,
			ks: transform((f) => f.right, 'R'),
			shapes: eyeShapes('right'),
		}),
		layer(3, 'Smile', 4, {
			parent: 4,
			ks: transform((f) => f.mouth, 'mouth'),
			shapes: [
				{
					...contour((f) => f.mouth.path, 'mouth.path', false, 5, mouthTolerance),
					nm: 'Smile centreline',
				},
				{
					ty: 'st',
					nm: 'Ink',
					c: { a: 0, k: INK },
					o: { a: 0, k: INK_OPACITY },
					w: { a: 0, k: MOUTH.width },
					lc: 2,
					lj: 2,
				},
			],
		}),
		...[
			['Left glow', 'left', 1, 5],
			['Right glow', 'right', 2, 6],
		].map(([name, side, parent, ind]) =>
			layer(ind, name, 4, {
				parent,
				ks: {
					// The layer's own opacity is the glow: it follows the eye it belongs to.
					o: property(rendered, (f) => [f[side].glow * 100], {
						tolerance: [0.4],
						decimals: 2,
						label: name + '.o',
					}),
					r: { a: 0, k: 0 },
					p: { a: 0, k: [rendered[0].frames[0][side].anchor[0], HALO.y, 0] },
					a: { a: 0, k: [0, 0, 0] },
					s: { a: 0, k: [100, 100, 100] },
				},
				shapes: [
					{
						ty: 'el',
						nm: 'Halo',
						p: { a: 0, k: [0, 0] },
						s: { a: 0, k: [HALO.radius * 2, HALO.radius * 2] },
						d: 1,
					},
					{
						ty: 'gf',
						nm: 'Light',
						o: { a: 0, k: 100 },
						r: 1,
						t: 2,
						s: { a: 0, k: [0, 0] },
						e: { a: 0, k: [HALO.radius, 0] },
						g: {
							p: 3,
							// White at three stops; only the alpha falls away from the centre.
							k: {
								a: 0,
								k: [
									0, 1, 1, 1, 0.5, 1, 1, 1, 1, 1, 1, 1, 0, HALO.peak, 0.5, HALO.shoulder, 1, 0,
								],
							},
						},
					},
				],
			}),
		),
		// The whole face moves as one: a null parent with its own pivot.
		layer(4, 'Face', 3, { ks: transform((f) => f.face, 'face', [margin, margin]) }),
	];
	return {
		v: '5.12.2',
		fr: 60,
		ip: 0,
		op: total,
		// Lottie clips to its composition; the margin leaves room for squash, bounce
		// and follow-through. The player's viewBox keeps the exact 256 × 175 face size.
		w: 256 + margin * 2,
		h: 175 + margin * 2,
		viewport: `${margin} ${margin} 256 175`,
		nm: 'Lumen — a little character',
		ddd: 0,
		assets: [],
		layers,
		markers,
	};
}
