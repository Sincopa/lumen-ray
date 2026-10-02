import { readFile } from 'node:fs/promises';

export const TAU = Math.PI * 2;
export const mix = (a, b, t) => a + (b - a) * t;
export const clamp = (value, lo, hi) => Math.min(hi, Math.max(lo, value));
const radians = (degrees) => (degrees * Math.PI) / 180;

/** The face is drawn in a 256 × 175 box; motion pivots about its optical centre. */
export const FACE = { width: 256, height: 175, pivot: [128, 94] };
export const EYE_CENTERS = [44.3682, 212.632];
/**
 * Squash, stretch and tilt of an eye all pivot about the dome's flat base, so an eye
 * grows upwards from its "feet" instead of swelling around its middle.
 */
const EYE_ANCHOR_Y = 76;

// ---------------------------------------------------------------------------
// Path helpers. A path is {c, v, i, o}: vertices plus *relative* tangent handles.
// ---------------------------------------------------------------------------

/** Parses the M/C/V/Z subset used by the supplied artwork into one cubic contour. */
export function parseCubicPath(source) {
	const tokens = source.match(/[MCVZ]|-?\d*\.?\d+(?:e[-+]?\d+)?/gi);
	const v = [],
		i = [],
		o = [];
	let cursor = 0,
		command;
	const point = () => [Number(tokens[cursor++]), Number(tokens[cursor++])];
	const add = (p, handle = [0, 0]) => {
		v.push(p);
		i.push(handle);
		o.push([0, 0]);
	};
	while (cursor < tokens.length) {
		if (/^[A-Z]$/i.test(tokens[cursor])) command = tokens[cursor++];
		if (command === 'M') {
			add(point());
			command = 'L';
		} else if (command === 'C') {
			const a = point(),
				b = point(),
				end = point(),
				start = v.at(-1);
			o[o.length - 1] = [a[0] - start[0], a[1] - start[1]];
			add(end, [b[0] - end[0], b[1] - end[1]]);
		} else if (command === 'V') {
			const y = Number(tokens[cursor++]);
			if (y !== v.at(-1)[1]) add([v.at(-1)[0], y]);
		} else if (command === 'Z') break;
		else throw new Error('Unsupported SVG command: ' + command);
	}
	if (v.at(-1).every((value, axis) => value === v[0][axis])) {
		i[0] = i.pop();
		v.pop();
		o.pop();
	}
	return { c: true, v, i, o };
}

const clonePath = (path) => ({
	c: path.c,
	v: path.v.map((p) => [...p]),
	i: path.i.map((p) => [...p]),
	o: path.o.map((p) => [...p]),
});

/** Adds scaled differences of several same-topology paths to a base path. */
function offsetPath(base, deltas) {
	const out = clonePath(base);
	for (const [target, weight] of deltas) {
		if (!weight) continue;
		for (const key of ['v', 'i', 'o'])
			target[key].forEach((point, index) => {
				point.forEach((value, axis) => {
					out[key][index][axis] += (value - base[key][index][axis]) * weight;
				});
			});
	}
	return out;
}

/**
 * Applies the same affine map Lottie builds from a layer's transform:
 * scale and rotate about the anchor, then translate. Handles are vectors.
 */
export function affine(path, { sx = 1, sy = 1, rot = 0, tx = 0, ty = 0, ax = 0, ay = 0 }) {
	const cos = Math.cos(rot),
		sin = Math.sin(rot);
	const linear = ([x, y]) => {
		const px = x * sx,
			py = y * sy;
		return [px * cos - py * sin, px * sin + py * cos];
	};
	const place = ([x, y]) => {
		const [lx, ly] = linear([x - ax, y - ay]);
		return [lx + ax + tx, ly + ay + ty];
	};
	return { c: path.c, v: path.v.map(place), i: path.i.map(linear), o: path.o.map(linear) };
}

const shift = (path, dx, dy) => ({
	c: path.c,
	v: path.v.map(([x, y]) => [x + dx, y + dy]),
	i: path.i,
	o: path.o,
});

// ---------------------------------------------------------------------------
// Eye key shapes. All share the artwork's vertex order (right, bottom, left, top)
// so any weighted sum of them is a valid, smooth contour.
// ---------------------------------------------------------------------------

const KAPPA = 0.5522847498;

const closedShape = (v, i, o) => ({ c: true, v, i, o });

function ellipse(rx, ry, cy) {
	const kx = KAPPA * rx,
		ky = KAPPA * ry;
	return closedShape(
		[
			[rx, cy],
			[0, cy + ry],
			[-rx, cy],
			[0, cy - ry],
		],
		[
			[0, -ky],
			[kx, 0],
			[0, ky],
			[-kx, 0],
		],
		[
			[0, ky],
			[-kx, 0],
			[0, -ky],
			[kx, 0],
		],
	);
}

function distanceToPolyline(point, line) {
	let best = Infinity;
	for (let n = 0; n < line.length - 1; n++) {
		const [ax, ay] = line[n],
			[bx, by] = line[n + 1];
		const dx = bx - ax,
			dy = by - ay;
		const len2 = dx * dx + dy * dy || 1e-9;
		const t = clamp(((point[0] - ax) * dx + (point[1] - ay) * dy) / len2, 0, 1);
		best = Math.min(best, Math.hypot(point[0] - (ax + dx * t), point[1] - (ay + dy * t)));
	}
	return best;
}

const bezierAt = (p0, p1, p2, p3, t) => {
	const m = 1 - t;
	return [
		m * m * m * p0[0] + 3 * m * m * t * p1[0] + 3 * m * t * t * p2[0] + t * t * t * p3[0],
		m * m * m * p0[1] + 3 * m * m * t * p1[1] + 3 * m * t * t * p2[1] + t * t * t * p3[1],
	];
};

/** Least-squares handle lengths for one cubic hugging a dense polyline. */
function fitHandles(piece, d0, d1) {
	const p0 = piece[0],
		p3 = piece.at(-1);
	const chord = Math.hypot(p3[0] - p0[0], p3[1] - p0[1]);
	const cost = (h1, h2) => {
		const c1 = [p0[0] + d0[0] * h1, p0[1] + d0[1] * h1];
		const c2 = [p3[0] - d1[0] * h2, p3[1] - d1[1] * h2];
		let sum = 0;
		for (let n = 1; n < 40; n++) {
			const d = distanceToPolyline(bezierAt(p0, c1, c2, p3, n / 40), piece);
			sum += d * d;
		}
		const curve = Array.from({ length: 81 }, (_, n) => bezierAt(p0, c1, c2, p3, n / 80));
		for (let n = 0; n < piece.length; n += 3) {
			const d = distanceToPolyline(piece[n], curve);
			sum += d * d;
		}
		return sum;
	};
	let best = { cost: Infinity, h1: chord / 3, h2: chord / 3 };
	let lo1 = 0.02,
		hi1 = 1.4,
		lo2 = 0.02,
		hi2 = 1.4;
	for (let pass = 0; pass < 4; pass++) {
		const steps = 16;
		for (let a = 0; a <= steps; a++)
			for (let b = 0; b <= steps; b++) {
				const h1 = (lo1 + ((hi1 - lo1) * a) / steps) * chord;
				const h2 = (lo2 + ((hi2 - lo2) * b) / steps) * chord;
				const c = cost(h1, h2);
				if (c < best.cost) best = { cost: c, h1, h2 };
			}
		const span1 = (hi1 - lo1) / 6,
			span2 = (hi2 - lo2) / 6;
		lo1 = Math.max(0.005, best.h1 / chord - span1);
		hi1 = best.h1 / chord + span1;
		lo2 = Math.max(0.005, best.h2 / chord - span2);
		hi2 = best.h2 / chord + span2;
	}
	return best;
}

/**
 * A closed eye is the same ribbon as the smile: a round-capped arc. The outline is
 * sampled densely and re-fitted onto the artwork's four-vertex topology, so it
 * morphs cleanly to and from the open dome.
 */
function ribbonEye({ span, tipY, apexY, thickness }) {
	const sag = tipY - apexY;
	const s = Math.sign(sag) || 1; // +1 arch (∩), -1 bowl (∪)
	const radius = (span * span + sag * sag) / (2 * Math.abs(sag));
	const r = thickness / 2;
	const alphaEnd = Math.asin(span / radius);
	const centre = [0, s > 0 ? apexY + radius : apexY - radius];
	const centreline = (alpha) => [
		centre[0] + radius * Math.sin(alpha),
		centre[1] + (s > 0 ? -radius : radius) * Math.cos(alpha),
	];
	const normal = (alpha) => [Math.sin(alpha), s > 0 ? -Math.cos(alpha) : Math.cos(alpha)];
	const tangent = (alpha) => [Math.cos(alpha), s * Math.sin(alpha)];
	const edge = (alpha, side) => {
		const p = centreline(alpha),
			n = normal(alpha);
		return [p[0] + n[0] * r * side, p[1] + n[1] * r * side];
	};
	const lowerSide = -s,
		upperSide = s;
	const samples = 90;
	const arc = (from, to, side) =>
		Array.from({ length: samples + 1 }, (_, n) => edge(mix(from, to, n / samples), side));
	const sweep = (from, to) => {
		let delta = to - from;
		while (delta > Math.PI) delta -= TAU;
		while (delta < -Math.PI) delta += TAU;
		return delta;
	};
	/** A semicircular cap: from one edge junction, over the tip, to the other. */
	const cap = (alpha, outward, fromSide, toSide) => {
		const c = centreline(alpha);
		const t = tangent(alpha);
		const nrm = normal(alpha);
		const a0 = Math.atan2(nrm[1] * fromSide, nrm[0] * fromSide);
		const a1 = Math.atan2(t[1] * outward, t[0] * outward);
		const a2 = Math.atan2(nrm[1] * toSide, nrm[0] * toSide);
		const out = [];
		for (let n = 0; n <= 45; n++) {
			const ang = a0 + (sweep(a0, a1) * n) / 45;
			out.push([c[0] + Math.cos(ang) * r, c[1] + Math.sin(ang) * r]);
		}
		for (let n = 1; n <= 45; n++) {
			const ang = a1 + (sweep(a1, a2) * n) / 45;
			out.push([c[0] + Math.cos(ang) * r, c[1] + Math.sin(ang) * r]);
		}
		return out;
	};
	// Clockwise on screen: right cap → lower edge → left cap → upper edge.
	const rightCap = cap(alphaEnd, 1, upperSide, lowerSide);
	const leftCap = cap(-alphaEnd, -1, lowerSide, upperSide);
	const pieces = [
		[...rightCap.slice(45), ...arc(alphaEnd, 0, lowerSide)], // right tip → lower apex
		[...arc(0, -alphaEnd, lowerSide), ...leftCap.slice(0, 46)], // lower apex → left tip
		[...leftCap.slice(45), ...arc(-alphaEnd, 0, upperSide)], // left tip → upper apex
		[...arc(0, alphaEnd, upperSide), ...rightCap.slice(0, 46)], // upper apex → right tip
	];
	const direction = (list) => {
		const a0 = list[0],
			a1 = list[Math.min(list.length - 1, 2)];
		const dx = a1[0] - a0[0],
			dy = a1[1] - a0[1],
			len = Math.hypot(dx, dy) || 1;
		return [dx / len, dy / len];
	};
	const dirs = pieces.map(direction);
	const v = [],
		i = [],
		o = [];
	for (let n = 0; n < 4; n++) {
		const next = (n + 1) % 4;
		const { h1, h2 } = fitHandles(pieces[n], dirs[n], dirs[next]);
		o[n] = [dirs[n][0] * h1, dirs[n][1] * h1];
		i[next] = [-dirs[next][0] * h2, -dirs[next][1] * h2];
		v[n] = pieces[n][0];
	}
	return closedShape(v, i, o);
}

/** Smug half-moon: flat lid, round cheek. */
const smugShape = closedShape(
	[
		[40, 63],
		[0, 88],
		[-40, 63],
		[0, 62],
	],
	[
		[0, -2],
		[22, 0],
		[0, 14],
		[-22, 0],
	],
	[
		[0, 14],
		[-22, 0],
		[0, -2],
		[22, 0],
	],
);

export async function loadArtwork() {
	const svg = await readFile(new URL('../../src/assets/lumen-face.svg', import.meta.url), 'utf8');
	const [left, right] = [...svg.matchAll(/<path d="([^"]+)"/g)].map((m) => parseCubicPath(m[1]));
	return { left, right };
}

const artwork = await loadArtwork();
const DOME = shift(artwork.left, -EYE_CENTERS[0], 0);

export const EYE_KEYS = {
	round: ellipse(30, 30, 66),
	wide: ellipse(26, 37, 62),
	smug: smugShape,
	closed: ribbonEye({ span: 30, tipY: 74, apexY: 59, thickness: 9 }),
	droop: ribbonEye({ span: 31, tipY: 76, apexY: 85, thickness: 8 }),
	happy: ribbonEye({ span: 27, tipY: 74, apexY: 52, thickness: 12.5 }),
};

/** Weighted blend of the dome with every key shape, in the eye's local frame. */
export function blendEye(weights) {
	return offsetPath(
		DOME,
		Object.entries(EYE_KEYS).map(([name, shape]) => [shape, weights[name] ?? 0]),
	);
}

// ---------------------------------------------------------------------------
// Glints: small highlights punched through the open eye (even-odd hole).
// ---------------------------------------------------------------------------

const GLINT_CENTER = [-9.5, 56];
const GLINT_RADIUS = 6.8;

function glintPath(amount) {
	const r = Math.max(0.001, GLINT_RADIUS * amount);
	return shift(ellipse(r, r, 0), GLINT_CENTER[0], GLINT_CENTER[1]);
}

// ---------------------------------------------------------------------------
// Mouth: an arc of controllable curvature, drawn as a round-capped stroke.
// ---------------------------------------------------------------------------

export const MOUTH = {
	apex: [128.565, 143.4],
	curvature: 1 / 56.35,
	half: 42.15,
	width: 10.82,
	/** Wavelength, in pixels, of the optional travelling ripple. */
	wavelength: 62,
};
/** The centre of the resting smile's circle: a "slide" turns the mouth about it. */
export const MOUTH_PIVOT = [MOUTH.apex[0], MOUTH.apex[1] - 1 / MOUTH.curvature];

function simpson(fn, a, b) {
	if (a === b) return [0, 0];
	const steps = Math.max(6, Math.ceil(Math.abs(b - a) / 2.5) * 2);
	const h = (b - a) / steps;
	let sx = 0,
		sy = 0;
	for (let n = 0; n <= steps; n++) {
		const [x, y] = fn(a + n * h);
		const weight = n === 0 || n === steps ? 1 : n % 2 ? 4 : 2;
		sx += weight * x;
		sy += weight * y;
	}
	return [(sx * h) / 3, (sy * h) / 3];
}

/**
 * Builds the mouth centreline from its curvature profile
 *   κ(s) = k0 + k1·s + wob·sin(2πs/λ + phase)
 * and re-expresses the visible span [s0, s1] with five cubic vertices.
 */
export function mouthPath({ tilt, k0, k1, wob, phase, s0, s1 }) {
	const lambda = MOUTH.wavelength;
	const heading = (s) =>
		tilt -
		k0 * s -
		(k1 * s * s) / 2 +
		wob * (lambda / TAU) * (Math.cos((TAU * s) / lambda + phase) - Math.cos(phase));
	const direction = (s) => [Math.cos(heading(s)), Math.sin(heading(s))];
	const at = (s) => {
		const [dx, dy] = simpson(direction, 0, s);
		return [MOUTH.apex[0] + dx, MOUTH.apex[1] + dy];
	};
	const knots = 5;
	const hi = Math.max(s1, s0 + 0.01);
	const stations = Array.from({ length: knots }, (_, n) => mix(s0, hi, n / (knots - 1)));
	const v = stations.map(at);
	const angles = stations.map(heading);
	const i = [],
		o = [];
	for (let n = 0; n < knots; n++) {
		const forward = n < knots - 1 ? segmentHandle(v, angles, n) : 0;
		const backward = n > 0 ? segmentHandle(v, angles, n - 1) : 0;
		const d = [Math.cos(angles[n]), Math.sin(angles[n])];
		o.push([d[0] * forward, d[1] * forward]);
		i.push([-d[0] * backward, -d[1] * backward]);
	}
	return { c: false, v, i, o };
}

/** Exact cubic handle length for a circular arc between two stations. */
function segmentHandle(points, angles, n) {
	const chord = Math.hypot(points[n + 1][0] - points[n][0], points[n + 1][1] - points[n][1]);
	const c = Math.cos((angles[n + 1] - angles[n]) / 4);
	return chord / (3 * c * c);
}

// ---------------------------------------------------------------------------
// Pose → layers
// ---------------------------------------------------------------------------

export const REST = {
	'face.x': 0,
	'face.y': 0,
	'face.roll': 0,
	'face.sx': 1,
	'face.sy': 1,
	'mouth.x': 0,
	'mouth.y': 0,
	'mouth.slide': 0,
	'mouth.tilt': 0,
	'mouth.k0': MOUTH.curvature,
	'mouth.k1': 0,
	'mouth.wob': 0,
	'mouth.phase': 0,
	'mouth.s0': -MOUTH.half,
	'mouth.s1': MOUTH.half,
};
for (const side of ['L', 'R'])
	Object.assign(REST, {
		[side + '.round']: 0,
		[side + '.wide']: 0,
		[side + '.smug']: 0,
		[side + '.closed']: 0,
		[side + '.droop']: 0,
		[side + '.happy']: 0,
		[side + '.dx']: 0,
		[side + '.dy']: 0,
		[side + '.sx']: 1,
		[side + '.sy']: 1,
		[side + '.rot']: 0,
		[side + '.glint']: 0,
		[side + '.glow']: 0,
	});
export const CHANNELS = Object.keys(REST);

/**
 * Springs may overshoot a shape weight; a little reads as squash and stretch, but
 * past ~12% a circle turns into an egg. This smoothly caps the extrapolation.
 */
const OVERSHOOT = { low: -0.15, high: 1.12 };
function limitWeight(weight) {
	if (weight > 1) return 1 + (OVERSHOOT.high - 1) * Math.tanh((weight - 1) / (OVERSHOOT.high - 1));
	if (weight < 0) return OVERSHOOT.low * Math.tanh(weight / OVERSHOOT.low);
	return weight;
}

/**
 * Splits a flat channel map into what the Lottie file animates: a transform for the
 * whole face (the parent layer), a transform plus contour for each eye and for the
 * mouth. Only genuine shape changes morph a contour; moving, squashing, tilting and
 * sliding are transforms, so they stay smooth and cheap.
 */
export function layersOf(p) {
	const eye = (index) => {
		const k = index === 0 ? 'L' : 'R';
		const weights = Object.fromEntries(
			['round', 'wide', 'smug', 'closed', 'droop', 'happy'].map((name) => [
				name,
				limitWeight(p[k + '.' + name]),
			]),
		);
		const occlusion = clamp(
			1 - 1.5 * (weights.closed + weights.droop + weights.happy + 0.7 * weights.smug),
			0,
			1,
		);
		// Light seeps through closed lids a little, so the glow dims as an eye shuts.
		const lidded = clamp(weights.droop + weights.closed + 0.5 * weights.happy, 0, 1);
		const cx = EYE_CENTERS[index];
		return {
			anchor: [cx, EYE_ANCHOR_Y],
			glow: clamp(p[k + '.glow'], 0, 1.4) * (1 - 0.7 * lidded),
			x: p[k + '.dx'],
			y: p[k + '.dy'],
			rot: p[k + '.rot'],
			sx: p[k + '.sx'],
			sy: p[k + '.sy'],
			outline: shift(blendEye(weights), cx, 0),
			glint: shift(glintPath(clamp(p[k + '.glint'], 0, 1.2) * occlusion), cx, 0),
		};
	};
	return {
		face: {
			anchor: FACE.pivot,
			x: p['face.x'],
			y: p['face.y'],
			rot: p['face.roll'],
			sx: p['face.sx'],
			sy: p['face.sy'],
		},
		left: eye(0),
		right: eye(1),
		mouth: {
			anchor: MOUTH_PIVOT,
			x: p['mouth.x'],
			y: p['mouth.y'],
			// A positive slide runs along the smile to the right: counter-clockwise.
			rot: -((p['mouth.slide'] * MOUTH.curvature * 180) / Math.PI),
			sx: 1,
			sy: 1,
			path: mouthPath({
				tilt: p['mouth.tilt'],
				k0: p['mouth.k0'],
				k1: p['mouth.k1'],
				wob: p['mouth.wob'],
				phase: p['mouth.phase'],
				s0: p['mouth.s0'],
				s1: p['mouth.s1'],
			}),
		},
	};
}

/** Applies one layer's transform, then its parent's, as the player will. */
const place = (path, layer, face) =>
	affine(
		affine(path, {
			sx: layer.sx,
			sy: layer.sy,
			rot: radians(layer.rot),
			tx: layer.x,
			ty: layer.y,
			ax: layer.anchor[0],
			ay: layer.anchor[1],
		}),
		{
			sx: face.sx,
			sy: face.sy,
			rot: radians(face.rot),
			tx: face.x,
			ty: face.y,
			ax: face.anchor[0],
			ay: face.anchor[1],
		},
	);

/** The contours of one frame in face coordinates — the ground truth for checks. */
export function absoluteOf(layers) {
	const { face, left, right, mouth } = layers;
	return {
		left: place(left.outline, left, face),
		leftGlint: place(left.glint, left, face),
		right: place(right.outline, right, face),
		rightGlint: place(right.glint, right, face),
		mouth: place(mouth.path, mouth, face),
	};
}

// ---------------------------------------------------------------------------
// Sampling (used by validation)
// ---------------------------------------------------------------------------

/** Dense polyline of a path, for containment and distance tests. */
export function samplePath(path, perSegment = 24) {
	const n = path.v.length;
	const count = path.c ? n : n - 1;
	const out = [];
	for (let s = 0; s < count; s++) {
		const a = path.v[s],
			b = path.v[(s + 1) % n];
		const c1 = [a[0] + path.o[s][0], a[1] + path.o[s][1]];
		const c2 = [b[0] + path.i[(s + 1) % n][0], b[1] + path.i[(s + 1) % n][1]];
		for (let t = 0; t < perSegment; t++) out.push(bezierAt(a, c1, c2, b, t / perSegment));
	}
	if (!path.c) out.push(path.v.at(-1));
	return out;
}

export function pointInPolygon([x, y], polygon) {
	let inside = false;
	for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
		const [xi, yi] = polygon[i],
			[xj, yj] = polygon[j];
		if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
	}
	return inside;
}

/** The supplied artwork, so previews can compare against the original SVG. */
export const ARTWORK = artwork;
