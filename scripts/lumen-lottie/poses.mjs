import { MOUTH } from './geometry.mjs';
import { P } from './motion.mjs';

export { P };

/**
 * Reusable pose parts. A pose is { eyes, L, R, face, mouth } (see resolvePose);
 * parts are merged with P(...) so expressions read like a recipe.
 */

const NO_SHAPE = { round: 0, wide: 0, smug: 0, closed: 0, droop: 0, happy: 0 };

/** One eye key shape at full strength; every other shape is switched off. */
export const shape = (name, amount = 1, extra = {}) => ({
	...NO_SHAPE,
	[name]: amount,
	...extra,
});

/**
 * A glance, as offsets from whatever pose the face is in: the eyes lead, the head and
 * mouth follow a fraction of it, and the far eye turns slightly narrower than the near.
 */
export const lookChannels = (x, y = 0) => ({
	'L.dx': x * 0.92,
	'R.dx': x * 1.08,
	'L.dy': y,
	'R.dy': y,
	'L.sx': -x * 0.008,
	'R.sx': x * 0.008,
	'face.x': x * 0.3,
	'face.y': y * 0.2,
	'face.roll': x * 0.28,
	'mouth.x': x * 0.45,
});

/** The same glance as an absolute pose part. */
export const look = (x, y = 0) => ({
	L: { dx: x * 0.92, dy: y, sx: 1 - x * 0.008 },
	R: { dx: x * 1.08, dy: y, sx: 1 + x * 0.008 },
	face: { x: x * 0.3, y: y * 0.2, roll: x * 0.28 },
	mouth: { x: x * 0.45 },
});

export const calm = {};

/** Relaxed, a little smug: heavy lids and a lopsided grin. */
export const confident = {
	L: shape('smug', 0.7, { glow: 0.75 }),
	R: shape('smug', 0.84, { glow: 0.75 }),
	mouth: { k0: 0.0208, k1: 0.00005, tilt: -0.07, x: 1.5 },
	face: { roll: -0.4 },
};

/** Both eyes shut — a relaxed blink (`droop`) or a squeeze (`closed`). */
export const lidsShut = (kind = 'droop', depth = 1) => ({
	eyes: shape(kind, depth, { dy: 2, sx: 1.03, sy: 0.96 }),
	mouth: { y: -0.8 },
	face: { y: 0.8 },
});

/** Eyes squeezed into happy arches under a broad grin. */
export const joy = {
	eyes: shape('happy', 1, { dy: -1.5, glow: 0.9 }),
	mouth: { k0: 0.0256, s0: -46, s1: 46, y: 0.5 },
	face: { y: -1 },
};

/** Bright, round, sparkling eyes: attentive and a little startled. */
export const alert = {
	L: shape('round', 0.9, { wide: 0.12, glint: 0.9, dy: -1.5 }),
	R: shape('round', 0.86, { wide: 0.16, glint: 0.8, dy: -1.5 }),
	mouth: { k0: 0.0195 },
	face: { y: -1 },
};

/**
 * The smile has shrunk to a dash that rides along the resting smile's own arc.
 * Sliding is a rotation about the arc's centre, so the dash needs the resting curve.
 */
export const dash = (centre = 0, length = 28) => ({
	mouth: { k0: MOUTH.curvature, k1: 0, tilt: 0, slide: centre, s0: -length / 2, s1: length / 2 },
});

/** Worried: tilted eyes, a down-turned mouth. */
export const sad = {
	L: { ...NO_SHAPE, droop: 0.16, rot: -10, dy: 3, sy: 0.96, glint: 0 },
	R: { ...NO_SHAPE, droop: 0.16, rot: 10, dy: 3, sy: 0.96, glint: 0 },
	// The faint ripple is the lip about to tremble.
	mouth: { k0: -0.016, s0: -34, s1: 34, y: -17.4, wob: 0.006 },
	face: { y: 2.4 },
};

/** Wide-eyed shock with a small round "o". */
export const startled = {
	L: shape('wide', 0.95, { glint: 0.85, dy: -2.5 }),
	R: shape('wide', 0.9, { glint: 0.8, dy: -2.5 }),
	mouth: { k0: 0.111, s0: -28, s1: 28, y: -12.4 },
	face: { y: -5.2, sy: 1.045, sx: 0.985 },
};

/**
 * The same pose seen in a mirror: the eyes swap sides and everything that leans
 * left leans right. Used to wink with either eye.
 */
export function mirror(spec) {
	const flip = (values, keys) => {
		if (!values) return values;
		const out = { ...values };
		for (const key of keys) if (key in out) out[key] = -out[key];
		return out;
	};
	const eye = (values) => flip(values, ['dx', 'rot']);
	const { L, R, face, mouth, eyes, ...rest } = spec;
	return {
		...rest,
		...(eyes ? { eyes: eye(eyes) } : {}),
		...(R ? { L: eye(R) } : {}),
		...(L ? { R: eye(L) } : {}),
		...(face ? { face: flip(face, ['x', 'roll']) } : {}),
		...(mouth
			? {
					mouth: {
						...flip(mouth, ['x', 'tilt', 'k1', 'slide']),
						...('s0' in mouth || 's1' in mouth
							? { s0: -(mouth.s1 ?? 0), s1: -(mouth.s0 ?? 0) }
							: {}),
					},
				}
			: {}),
	};
}
