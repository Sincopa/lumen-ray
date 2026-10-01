import { readFile, writeFile } from 'node:fs/promises';

// Retain the supplied artwork at rest and identical cubic topology in every pose.
const svg = await readFile(
	new URL('../src/assets/lumen-face.svg', import.meta.url),
	'utf8',
);
const paths = [...svg.matchAll(/<path d="([^"]+)"/g)].map((match) => match[1]);
function bezierPath(source) {
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
	return { i, o, v, c: true };
}
const artwork = paths.map(bezierPath);
const mix = (a, b, amount) => a + (b - a) * amount;
function blend(a, b, amount) {
	return {
		c: true,
		...Object.fromEntries(
			['v', 'i', 'o'].map((key) => [
				key,
				a[key].map((point, index) =>
					point.map((value, axis) => mix(value, b[key][index][axis], amount)),
				),
			]),
		),
	};
}
function mapPath(path, transform) {
	const v = path.v.map(transform);
	return {
		c: true,
		v,
		...Object.fromEntries(
			['i', 'o'].map((key) => [
				key,
				path[key].map((handle, index) => {
					const absolute = transform(
						handle.map((value, axis) => value + path.v[index][axis]),
					);
					return absolute.map((value, axis) => value - v[index][axis]);
				}),
			]),
		),
	};
}
function eyeShape(index, { round = 0, lid = 0, close = 0 } = {}) {
	const cx = index === 0 ? 44.3682 : 212.632,
		cy = 66;
	const circle = {
		c: true,
		v: [
			[cx + 30, cy],
			[cx, cy + 30],
			[cx - 30, cy],
			[cx, cy - 30],
		],
		i: [
			[0, -16.5685],
			[16.5685, 0],
			[0, 16.5685],
			[-16.5685, 0],
		],
		o: [
			[0, 16.5685],
			[-16.5685, 0],
			[0, -16.5685],
			[16.5685, 0],
		],
	};
	const confident = {
		c: true,
		v: [
			[cx + 40, cy - 3],
			[cx, cy + 22],
			[cx - 40, cy - 3],
			[cx, cy - 4],
		],
		i: [
			[0, -2],
			[22, 0],
			[0, 14],
			[-22, 0],
		],
		o: [
			[0, 14],
			[-22, 0],
			[0, -2],
			[22, 0],
		],
	};
	// A curved eyelid ribbon, rather than a vertically squashed open eye.
	const closed = {
		c: true,
		v: [
			[cx + 36, cy + 10],
			[cx, cy - 4],
			[cx - 36, cy + 10],
			[cx, cy - 12],
		],
		i: [
			[3, -2],
			[17, 0],
			[3, -2],
			[-20, 0],
		],
		o: [
			[-3, 3],
			[-17, 0],
			[-3, -2],
			[20, 0],
		],
	};
	return blend(
		blend(blend(artwork[index], circle, round), confident, lid),
		closed,
		close,
	);
}
function shape(index, pose) {
	const {
		gaze = [0, 0],
		lift = 0,
		roll = 0,
		spread = 0,
		grin = [1, 1, 0, 0],
	} = pose;
	let path =
		index < 2
			? eyeShape(index, pose[index === 0 ? 'left' : 'right'])
			: artwork[index];
	if (index === 2) {
		const [width, depth, tilt, shift] = grin;
		const contour = ([x, y]) => {
			const distance = (x - 128.565) / 44;
			return [
				128.565 + (x - 128.565) * width + shift + gaze[0] * 0.35,
				y +
					(depth - 1) * 19 * (1 - Math.min(1, Math.abs(distance)) ** 2) +
					tilt * distance,
			];
		};
		// Bend the centreline while preserving stroke thickness and tangent
		// continuity. Mapping handle endpoints through a parabola makes cusps.
		path = {
			c: true,
			v: path.v.map(contour),
			...Object.fromEntries(
				['i', 'o'].map((key) => [
					key,
					path[key].map(([x, y], point) => {
						const distance = (path.v[point][0] - 128.565) / 44;
						const slope =
							Math.abs(distance) < 1
								? (-2 * (depth - 1) * 19 * distance) / 44
								: 0;
						return [x * width, y + (slope + tilt / 44) * x];
					}),
				]),
			),
		};
	} else
		path = mapPath(path, ([x, y]) => [
			x + gaze[0] + (index ? spread : -spread),
			y + gaze[1],
		]);
	const angle = (roll * Math.PI) / 180;
	return mapPath(path, ([x, y]) => [
		128 + (x - 128) * Math.cos(angle) - (y - 94) * Math.sin(angle),
		94 + (x - 128) * Math.sin(angle) + (y - 94) * Math.cos(angle) + lift,
	]);
}
const neutral = {};
const confident = {
	left: { lid: 0.68 },
	right: { lid: 0.8 },
	grin: [0.98, 1.18, -5, 4],
};
// Holds give each look time to read; anticipation and smaller settling poses add weight.
const sequences = {
	idle: {
		loop: true,
		frames: [
			[0, neutral],
			[60, neutral],
			[
				87,
				{
					gaze: [-4, -1],
					left: { lid: 0.08 },
					right: { lid: 0.18 },
					grin: [1, 1.04, 2, -2],
					roll: -1,
				},
			],
			[
				138,
				{
					gaze: [-4, -1],
					left: { lid: 0.08 },
					right: { lid: 0.18 },
					grin: [1, 1.04, 2, -2],
					roll: -1,
				},
			],
			[
				170,
				{
					gaze: [3, -2],
					left: { round: 0.2 },
					right: { round: 0.3 },
					grin: [1.03, 1.15, -2, 1],
					lift: -1,
				},
			],
			[
				204,
				{
					gaze: [3, -2],
					left: { round: 0.2 },
					right: { round: 0.3 },
					grin: [1.03, 1.15, -2, 1],
					lift: -1,
				},
			],
			[227, neutral],
			[257, neutral],
			[
				262,
				{
					left: { close: 1 },
					right: { close: 0.97 },
					grin: [1.02, 1.12, 0, 0],
					lift: 1,
				},
			],
			[267, { left: { round: 0.08 }, right: { close: 0.1 }, lift: -0.8 }],
			[280, neutral],
			[390, neutral],
			[
				411,
				{
					gaze: [0, -3],
					left: { round: 0.18 },
					right: { round: 0.18 },
					grin: [1.02, 1.08, 0, 0],
					lift: -1.2,
				},
			],
			[
				440,
				{
					gaze: [0, -3],
					left: { round: 0.18 },
					right: { round: 0.18 },
					grin: [1.02, 1.08, 0, 0],
					lift: -1.2,
				},
			],
			[464, neutral],
			[561, neutral],
			[566, { left: { close: 0.95 }, right: { close: 1 }, lift: 1 }],
			[573, { left: { round: 0.06 }, right: { round: 0.06 }, lift: -0.5 }],
			[588, neutral],
			[720, neutral],
		],
	},
	connecting: {
		loop: true,
		frames: [
			[0, neutral],
			[
				10,
				{
					left: { lid: 0.35 },
					right: { lid: 0.35 },
					grin: [0.94, 0.75, 0, 0],
					lift: 3,
				},
			],
			[
				27,
				{
					left: { round: 0.8 },
					right: { round: 0.8 },
					gaze: [-4, -2],
					grin: [0.9, 1.15, 3, -3],
					lift: -2,
					roll: -1.4,
				},
			],
			[
				47,
				{
					left: { round: 0.8 },
					right: { round: 0.8 },
					gaze: [-4, -2],
					grin: [0.9, 1.15, 3, -3],
					lift: -2,
					roll: -1.4,
				},
			],
			[
				76,
				{
					left: { round: 0.65 },
					right: { round: 0.7, lid: 0.12 },
					gaze: [5, -1],
					grin: [0.98, 1.25, -4, 4],
					roll: 1.5,
				},
			],
			[
				98,
				{
					left: { round: 0.65 },
					right: { round: 0.7, lid: 0.12 },
					gaze: [5, -1],
					grin: [0.98, 1.25, -4, 4],
					roll: 1.5,
				},
			],
			[
				103,
				{
					left: { close: 1 },
					right: { close: 1 },
					grin: [1.02, 1.3, -2, 2],
					lift: 2,
				},
			],
			[
				115,
				{
					left: { round: 0.35 },
					right: { round: 0.35 },
					grin: [1.04, 1.22, 0, 0],
					lift: -3,
				},
			],
			[144, neutral],
			[180, neutral],
		],
	},
	connected: {
		loop: false,
		then: 'connected-loop',
		frames: [
			[0, neutral],
			[
				8,
				{
					left: { lid: 0.38 },
					right: { lid: 0.38 },
					grin: [0.94, 0.82, 0, 0],
					lift: 3,
				},
			],
			[
				22,
				{
					left: { round: 0.6 },
					right: { round: 0.6 },
					grin: [1.1, 1.38, -2, 1],
					lift: -5,
					roll: -1.2,
				},
			],
			[
				38,
				{
					left: { lid: 0.15 },
					right: { close: 1 },
					grin: [0.98, 1.35, -8, 5],
					lift: -1,
					roll: -1.5,
				},
			],
			[
				48,
				{
					left: { lid: 0.15 },
					right: { close: 1 },
					grin: [0.98, 1.35, -8, 5],
					lift: -1,
					roll: -1.5,
				},
			],
			[
				66,
				{
					left: { lid: 0.36 },
					right: { lid: 0.46 },
					grin: [1.05, 1.16, -3.5, 3],
					lift: 0.8,
					roll: 0.4,
				},
			],
			[90, confident],
		],
	},
	'connected-loop': {
		loop: true,
		frames: [
			[0, confident],
			[90, confident],
			[
				96,
				{ ...confident, left: { close: 1 }, right: { close: 0.95 }, lift: 0.8 },
			],
			[
				104,
				{ ...confident, left: { lid: 0.2 }, right: { lid: 0.3 }, lift: -0.6 },
			],
			[120, confident],
			[210, confident],
			[
				238,
				{ ...confident, gaze: [-3, -1], roll: -0.8, grin: [1.04, 1.16, -2, 1] },
			],
			[
				270,
				{ ...confident, gaze: [-3, -1], roll: -0.8, grin: [1.04, 1.16, -2, 1] },
			],
			[298, confident],
			[380, confident],
			[386, { ...confident, left: { close: 0.96 }, right: { close: 1 } }],
			[395, confident],
			[480, confident],
		],
	},
	wink: {
		loop: false,
		frames: [
			[0, neutral],
			[
				7,
				{
					gaze: [2, -1],
					left: { round: 0.12 },
					right: { round: 0.12 },
					grin: [1.02, 1.16, -3, 2],
					lift: -1,
				},
			],
			[
				17,
				{
					left: { lid: 0.08 },
					right: { close: 1 },
					grin: [0.98, 1.3, -7, 4],
					lift: -2,
					roll: -1.7,
				},
			],
			[
				30,
				{
					left: { lid: 0.08 },
					right: { close: 1 },
					grin: [0.98, 1.3, -7, 4],
					lift: -2,
					roll: -1.7,
				},
			],
			[
				43,
				{
					left: { round: 0.1 },
					right: { round: 0.22 },
					grin: [1.03, 1.12, 1, 0],
					lift: 0.8,
					roll: 0.5,
				},
			],
			[60, neutral],
		],
	},
	error: {
		loop: false,
		frames: [
			[0, neutral],
			[
				14,
				{
					left: { round: 0.6 },
					right: { round: 0.65 },
					grin: [0.78, 0.35, 0, 0],
					lift: -2,
				},
			],
			[
				31,
				{
					left: { lid: 0.52 },
					right: { lid: 0.42 },
					grin: [0.84, -0.55, 0, 0],
					gaze: [0, 2],
					lift: 2,
					roll: -2,
				},
			],
			[
				42,
				{
					left: { lid: 0.52 },
					right: { lid: 0.42 },
					grin: [0.84, -0.55, 0, 0],
					gaze: [0, 2],
					lift: 2,
					roll: 1.6,
				},
			],
			[
				53,
				{
					left: { lid: 0.52 },
					right: { lid: 0.42 },
					grin: [0.84, -0.55, 0, 0],
					gaze: [0, 2],
					lift: 2,
					roll: -0.6,
				},
			],
			[
				75,
				{
					left: { lid: 0.52 },
					right: { lid: 0.42 },
					grin: [0.84, -0.55, 0, 0],
					gaze: [0, 2],
					lift: 2,
				},
			],
			[108, neutral],
			[120, neutral],
		],
	},
	disconnecting: {
		loop: false,
		frames: [
			[0, confident],
			[
				15,
				{
					left: { close: 1 },
					right: { close: 1 },
					grin: [1.04, 1.3, 0, 0],
					lift: 2,
				},
			],
			[
				28,
				{
					left: { round: 0.1 },
					right: { round: 0.1 },
					grin: [1.02, 1.08, 0, 0],
					lift: -1,
				},
			],
			[54, neutral],
			[66, neutral],
		],
	},
};
// The connected hover returns to its confident pose without a neutral-frame jump.
sequences['wink-connected'] = {
	loop: false,
	frames: sequences.wink.frames.map(([frame, pose]) => [
		frame,
		frame === 0 || frame === 60 ? confident : pose,
	]),
};
let cursor = 0;
const markers = [],
	frames = [];
for (const [name, sequence] of Object.entries(sequences)) {
	const duration = sequence.frames.at(-1)[0];
	markers.push({
		cm: name,
		tm: cursor,
		dr: duration,
		loop: sequence.loop,
		...(sequence.then ? { then: sequence.then } : {}),
	});
	for (const [frame, pose] of sequence.frames)
		frames.push([cursor + frame, pose]);
	// One unused frame separates different poses at a segment boundary.
	cursor += duration + 1;
}
const layers = artwork.map((_, index) => ({
	ddd: 0,
	ind: index + 1,
	ty: 4,
	nm: ['Left eye', 'Right eye', 'Smile'][index],
	ks: {
		o: { a: 0, k: 100 },
		r: { a: 0, k: 0 },
		p: { a: 0, k: [8, 8, 0] },
		a: { a: 0, k: [0, 0, 0] },
		s: { a: 0, k: [100, 100, 100] },
	},
	shapes: [
		{
			ty: 'sh',
			nm: 'Expression contour',
			ks: {
				a: 1,
				k: frames.map(([t, pose], frame) => ({
					t,
					s: [shape(index, pose)],
					...(frame < frames.length - 1
						? {
								e: [shape(index, frames[frame + 1][1])],
								o: { x: 0.42, y: 0 },
								i: { x: 0.18, y: 1 },
							}
						: {}),
				})),
			},
		},
		{
			ty: 'fl',
			c: { a: 0, k: [0, 0, 0, 1] },
			o: { a: 0, k: 67 },
			r: 1,
			nm: 'Original ink',
		},
	],
	ip: 0,
	op: cursor,
	st: 0,
	bm: 0,
}));
// Five decimals preserve the SVG while keeping a compact, entirely offline asset.
await writeFile(
	new URL('../src/assets/lumen-expressions.json', import.meta.url),
	JSON.stringify(
		{
			v: '5.12.2',
			fr: 60,
			ip: 0,
			op: cursor,
			// Lottie clips to its composition. Overscan accommodates the gaze and
			// follow-through; the player's viewBox preserves the exact design size.
			w: 272,
			h: 191,
			viewport: '8 8 256 175',
			nm: 'Lumen — a little character',
			ddd: 0,
			assets: [],
			layers,
			markers,
		},
		(_, value) =>
			typeof value === 'number' ? Math.round(value * 100000) / 100000 : value,
	),
);
console.log(
	'Generated six expressions and a connected resting loop from the original SVG.',
);
