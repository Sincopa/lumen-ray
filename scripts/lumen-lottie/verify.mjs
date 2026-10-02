import { absoluteOf, affine, layersOf } from './geometry.mjs';

/**
 * Re-evaluates a generated Lottie file the way a player would — keyframes, bezier
 * easing, layer transforms, parenting — and compares every baked frame against the
 * generator's own ideal geometry. This proves the keyframe fitting is faithful.
 */

const radians = (degrees) => (degrees * Math.PI) / 180;
const bezier = (s, a, b) => 3 * (1 - s) * (1 - s) * s * a + 3 * (1 - s) * s * s * b + s * s * s;

function easing(key) {
	const pick = (value) => (Array.isArray(value) ? value[0] : value);
	const x1 = pick(key.o.x),
		y1 = pick(key.o.y),
		x2 = pick(key.i.x),
		y2 = pick(key.i.y);
	if (x1 === y1 && x2 === y2) return (u) => u;
	return (u) => {
		let lo = 0,
			hi = 1;
		for (let n = 0; n < 40; n++) {
			const mid = (lo + hi) / 2;
			if (bezier(mid, x1, x2) < u) lo = mid;
			else hi = mid;
		}
		return bezier((lo + hi) / 2, y1, y2);
	};
}

const mixNumber = (a, b, p) => a + (b - a) * p;
function mixValue(a, b, p) {
	if (typeof a === 'number') return mixNumber(a, b, p);
	if (Array.isArray(a)) return a.map((value, index) => mixValue(value, b[index], p));
	return {
		c: a.c,
		v: mixValue(a.v, b.v, p),
		i: mixValue(a.i, b.i, p),
		o: mixValue(a.o, b.o, p),
	};
}

/** A property's value at a composition frame. */
export function valueAt(property, frame) {
	if (!property.a) return property.k;
	const keys = property.k;
	if (frame <= keys[0].t) return keys[0].s;
	if (frame >= keys.at(-1).t) return keys.at(-1).s;
	let lo = 0,
		hi = keys.length - 1;
	while (hi - lo > 1) {
		const mid = (lo + hi) >> 1;
		if (keys[mid].t <= frame) lo = mid;
		else hi = mid;
	}
	const a = keys[lo],
		b = keys[hi];
	const progress = easing(a)((frame - a.t) / (b.t - a.t));
	return mixValue(a.s, b.s, progress);
}

const scalar = (property, frame) => {
	const value = valueAt(property, frame);
	return Array.isArray(value) ? value[0] : value;
};

function transformAt(ks, frame) {
	const [ax, ay] = ks.a.k;
	const scale = valueAt(ks.s, frame);
	return {
		sx: scale[0] / 100,
		sy: scale[1] / 100,
		rot: radians(scalar(ks.r, frame)),
		tx: scalar(ks.p.x, frame) - ax,
		ty: scalar(ks.p.y, frame) - ay,
		ax,
		ay,
	};
}

const pathOf = (value) => (Array.isArray(value) ? value[0] : value);

/** The absolute contours a player would draw for one composition frame. */
export function drawn(json, frame) {
	const byName = Object.fromEntries(json.layers.map((layer) => [layer.nm, layer]));
	const face = transformAt(byName.Face.ks, frame);
	const place = (layer, shape) =>
		affine(affine(pathOf(valueAt(shape.ks, frame)), transformAt(layer.ks, frame)), face);
	const eye = (layer) => ({
		outline: place(layer, layer.shapes[0]),
		glint: place(layer, layer.shapes[1]),
	});
	return {
		left: eye(byName['Left eye']),
		right: eye(byName['Right eye']),
		mouth: place(byName.Smile, byName.Smile.shapes[0]),
	};
}

/** Every point that defines a contour: vertices and the ends of both handles. */
const points = (path) =>
	path.v.flatMap((vertex, n) => [
		vertex,
		[vertex[0] + path.i[n][0], vertex[1] + path.i[n][1]],
		[vertex[0] + path.o[n][0], vertex[1] + path.o[n][1]],
	]);

const deviation = (a, b) => {
	const pa = points(a),
		pb = points(b);
	let worst = 0;
	for (let n = 0; n < pa.length; n++)
		worst = Math.max(worst, Math.hypot(pa[n][0] - pb[n][0], pa[n][1] - pb[n][1]));
	return worst;
};

/**
 * @param json    the generated composition
 * @param baked   [{ name, frames: channelMap[] }] — what the storyboard simulated
 * @returns the worst deviation, in design px, per segment
 */
export function verify(json, baked) {
	const margin = (json.w - 256) / 2;
	const report = {};
	for (const segment of baked) {
		const marker = json.markers.find((m) => m.cm === segment.name);
		let worst = 0,
			at = 0;
		segment.frames.forEach((channels, index) => {
			const ideal = absoluteOf(layersOf(channels));
			const actual = drawn(json, marker.tm + index);
			const shift = (path) => ({
				...path,
				v: path.v.map(([x, y]) => [x + margin, y + margin]),
			});
			const pairs = [
				[shift(ideal.left), actual.left.outline],
				[shift(ideal.right), actual.right.outline],
				[shift(ideal.mouth), actual.mouth],
			];
			// A hidden glint is a sub-pixel dot, so its exact whereabouts do not matter.
			const visible = (path) => path.i.some(([x, y]) => Math.abs(x) + Math.abs(y) > 0.05);
			if (visible(ideal.leftGlint)) pairs.push([shift(ideal.leftGlint), actual.left.glint]);
			if (visible(ideal.rightGlint)) pairs.push([shift(ideal.rightGlint), actual.right.glint]);
			for (const [a, b] of pairs) {
				const d = deviation(a, b);
				if (d > worst) {
					worst = d;
					at = index;
				}
			}
		});
		report[segment.name] = { worst, at };
	}
	return report;
}
