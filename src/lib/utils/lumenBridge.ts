const NUMBER = /-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/gi;
const numbers = (value: string) => (value.match(NUMBER) ?? []).map(Number);
const outline = (value: string) => value.replace(NUMBER, '#');

/** Corners of the area the face can occupy, in the composition's own units. */
const REACH: [number, number][] = [
	[0, 0],
	[320, 0],
	[0, 240],
	[320, 240],
];

/**
 * How far apart, in px, two renderings of one layer transform (`matrix(a b c d e f)`),
 * one contour (`path("M…")`) or one layer opacity are. Contours that do not share a structure cannot be
 * morphed and report Infinity.
 */
export function displacement(
	property: 'transform' | 'd' | 'opacity',
	a: string,
	b: string,
): number {
	// A light fading from fully on to fully off counts as a 60 px move.
	if (property === 'opacity') return Math.abs(Number(a) - Number(b)) * 60;
	if (property === 'd') {
		if (outline(a) !== outline(b)) return Infinity;
		const x = numbers(a),
			y = numbers(b);
		return x.reduce(
			(worst, value, index) => Math.max(worst, Math.abs(value - y[index])),
			0,
		);
	}
	const x = numbers(a),
		y = numbers(b);
	if (x.length !== 6 || y.length !== 6) return Infinity;
	const at = (m: number[], px: number, py: number) => [
		m[0] * px + m[2] * py + m[4],
		m[1] * px + m[3] * py + m[5],
	];
	return REACH.reduce((worst, [px, py]) => {
		const from = at(x, px, py),
			to = at(y, px, py);
		return Math.max(worst, Math.hypot(from[0] - to[0], from[1] - to[1]));
	}, 0);
}

/**
 * How long to take over closing a gap of `gap` px. A face drifting a pixel or so (the
 * breath, say) is simply cut over, so a reaction starts at once; bigger gaps take longer
 * the further the face has to go, up to `limit` ms.
 */
export function bridgeDuration(gap: number, limit: number): number {
	return gap < 1.5 ? 0 : Math.min(limit, 40 + gap * 8);
}
