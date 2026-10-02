import { P, offsetPose } from './motion.mjs';
import {
	alert,
	calm,
	confident,
	dash,
	joy,
	lidsShut,
	look,
	lookChannels,
	mirror,
	sad,
	shape,
	startled,
} from './poses.mjs';

const TAU = Math.PI * 2;

// Spring recipes per group. The eyes lead, the head follows, the mouth trails: that
// small, consistent lag is what keeps the face from moving like a single sticker.
const LEAD = { L: 'snap', R: 'snap', face: 'soft', mouth: 'glide' };
const POP = { L: 'pop', R: 'pop', face: 'soft', mouth: 'soft' };
const SETTLE = { L: 'crisp', R: 'crisp', face: 'soft', mouth: 'soft' };
const SHUT = { L: 'snap', R: 'snap', face: 'crisp', mouth: 'crisp' };
const EASE = { L: 'glide', R: 'glide', face: 'slow', mouth: 'glide' };
const SLOW = { L: 'slow', R: 'slow', face: 'slow', mouth: 'slow' };
const CASUAL = { L: 'crisp', R: 'crisp', face: 'soft', mouth: 'glide' };
const BOUNCE = { L: 'pep', R: 'pep', face: 'pop', mouth: 'pep' };

/** A slow, almost imperceptible breath that gives the face weight. */
const breath =
	(amount = 1) =>
	(t, period) => {
		const s = Math.sin((TAU * t) / period);
		return {
			'face.y': -0.62 * amount * s,
			'face.sy': 0.0045 * amount * s,
			'face.sx': -0.002 * amount * s,
			'L.dy': 0.22 * amount * s,
			'R.dy': 0.22 * amount * s,
			'mouth.k0': 0.00035 * amount * s,
		};
	};

const sum = (...maps) => {
	const out = {};
	for (const map of maps)
		for (const [key, value] of Object.entries(map)) out[key] = (out[key] ?? 0) + value;
	return out;
};

// ---------------------------------------------------------------------------
// Gestures. Each begins and ends at its mood's rest pose, so it chains onto the
// mood's loop — the runtime sprinkles them in at random.
// ---------------------------------------------------------------------------

function gestureSet(mood, rest) {
	const g = (name, duration, keys, weight = 1) => ({
		name: `${mood}:${name}`,
		kind: 'gesture',
		mood,
		weight,
		timeline: { duration, keys: [{ at: 0, pose: rest }, ...keys] },
	});
	const shut = P(rest, lidsShut());
	const heavy = mood === 'connected';
	return [
		// Heavy-lidded (connected) blinks are slower and lazier.
		g(
			'blink',
			heavy ? 0.66 : 0.46,
			[
				{ at: 0.02, pose: shut, spring: heavy ? EASE : SHUT, lag: { R: 0.012 } },
				{ at: heavy ? 0.26 : 0.115, pose: rest, spring: POP, lag: { R: 0.012 } },
			],
			9,
		),
		g(
			'blink-double',
			0.92,
			[
				{ at: 0.02, pose: shut, spring: SHUT },
				{ at: 0.1, pose: rest, spring: POP },
				{ at: 0.3, pose: shut, spring: SHUT },
				{ at: 0.39, pose: rest, spring: POP, lag: { R: 0.01 } },
			],
			1.5,
		),
		...[
			['look-left', 1.75, look(-6.2, -0.8), { eyes: { round: 0.12 } }, 0.05, false],
			['look-right', 1.6, look(5.6, -0.6), { eyes: { round: 0.1 } }, 0.04, true],
		].map(([name, duration, glance, shape2, at, blink]) => {
			const aside = P(rest, glance, shape2);
			return g(
				name,
				duration,
				[
					{ at, pose: aside, spring: heavy ? CASUAL : LEAD, lag: blink ? { R: 0.01 } : {} },
					// The blink comes as the gaze lingers, then the eyes come home.
					{ at: 0.8, pose: P(aside, lidsShut()), spring: SHUT },
					{ at: 0.9, pose: aside, spring: POP },
					{ at: 1.12, pose: rest, spring: SETTLE },
				],
				1.6,
			);
		}),
		g(
			'look-up',
			1.8,
			[
				{
					at: 0.05,
					pose: P(rest, {
						eyes: { round: 0.3, wide: 0.12, dy: -4.2 },
						face: { y: -1.2 },
						mouth: { k0: 0.0192 },
					}),
					spring: { L: 'snap', R: 'snap', face: 'soft', mouth: 'soft' },
				},
				{ at: 1.12, pose: rest, spring: SETTLE },
			],
			0.9,
		),
		g(
			'smirk',
			1.5,
			[
				{
					at: 0.06,
					pose: P(rest, {
						L: { round: 0.14, dy: -0.8 },
						R: { smug: 0.38, dy: 0.8 },
						mouth: { tilt: -0.12, k1: 0.0001, k0: 0.0202 },
						face: { roll: -1.2 },
					}),
					spring: SETTLE,
				},
				{ at: 1.0, pose: rest, spring: SETTLE },
			],
			0.9,
		),
		g(
			'nod',
			0.95,
			[
				{
					at: 0.05,
					pose: P(rest, {
						face: { y: 3.2 },
						eyes: { droop: 0.35, dy: 1.5 },
						mouth: { k0: 0.0186 },
					}),
					spring: { L: 'crisp', R: 'crisp', face: 'crisp', mouth: 'soft' },
				},
				{
					at: 0.24,
					pose: P(rest, { face: { y: -1 }, mouth: { k0: 0.0198 } }),
					spring: { L: 'pop', R: 'pop', face: 'pop', mouth: 'soft' },
				},
				{ at: 0.5, pose: rest, spring: { L: 'soft', R: 'soft', face: 'soft', mouth: 'soft' } },
			],
			0.9,
		),
		// The rest of the repertoire is for a face that is just sitting there.
		...(heavy
			? []
			: [
					g(
						'smile',
						1.6,
						[
							{
								at: 0.06,
								pose: P(rest, {
									eyes: shape('happy', 0.55, { dy: -0.5 }),
									mouth: { k0: 0.0235, s0: -45, s1: 45 },
									face: { y: -0.8 },
								}),
								spring: SETTLE,
							},
							{ at: 1.0, pose: rest, spring: SETTLE },
						],
						0.9,
					),
					g(
						'look-around',
						2.6,
						[
							{
								at: 0.05,
								pose: P(rest, look(-6, -0.6), { eyes: { round: 0.1 } }),
								spring: LEAD,
							},
							{
								at: 0.85,
								pose: P(rest, look(6, -0.4), { eyes: { round: 0.1 } }),
								spring: { L: 'crisp', R: 'crisp', face: 'soft', mouth: 'glide' },
							},
							{ at: 1.7, pose: rest, spring: SETTLE },
						],
						0.8,
					),
					// A big, sleepy yawn: the smile curls into an "o" while the eyes squeeze shut.
					g(
						'yawn',
						2.7,
						[
							{
								at: 0.06,
								pose: P(rest, {
									eyes: shape('droop', 0.7, { dy: 1.5 }),
									face: { y: 1.2 },
									mouth: { k0: 0.0176 },
								}),
								spring: EASE,
							},
							{
								at: 0.42,
								pose: P(rest, {
									eyes: shape('closed', 1, { dy: 2, sy: 0.96 }),
									mouth: { k0: 0.068, s0: -43, s1: 43, y: -7.7 },
									face: { y: -3, sy: 1.035, sx: 0.985 },
								}),
								spring: { L: 'glide', R: 'glide', face: 'soft', mouth: 'soft' },
							},
							{
								at: 1.6,
								pose: P(rest, {
									eyes: shape('droop', 0.5, { dy: 1 }),
									face: { y: 0.8 },
									mouth: { k0: 0.0185 },
								}),
								spring: EASE,
							},
							{ at: 1.95, pose: rest, spring: POP },
						],
						0.4,
					),
				]),
	];
}

// ---------------------------------------------------------------------------
// The scanner: while connecting, the smile shrinks to a dash that runs back and
// forth along its own arc and the eyes follow it.
// ---------------------------------------------------------------------------

/**
 * The flinch of being pressed: a squint and a small crouch. A connect or disconnect
 * click starts from this pose, so the press and the reaction are one continuous move.
 */
const pressed = (rest) =>
	P(rest, {
		eyes: shape('smug', 0.5, { dy: 1.6 }),
		face: { y: 2, sy: 0.988, sx: 1.008 },
		mouth: { k0: 0.0212, s0: -35, s1: 35 },
	});
const FLINCH = { L: 'snap', R: 'snap', face: 'crisp', mouth: 'crisp' };

const SCAN_PERIOD = 1.2;
const SCAN_DASH = 40;
const scanBase = P(alert, dash(0, SCAN_DASH));

function scan(t) {
	const w = (TAU * t) / SCAN_PERIOD;
	// The dash stretches while it runs and tightens as it turns around.
	const squeeze = 12 * Math.sin(w) ** 2;
	const gaze = 5.2 * Math.sin(w - 0.45);
	return sum(lookChannels(gaze, 0), {
		'mouth.slide': 24 * Math.sin(w),
		'mouth.s0': squeeze / 2,
		'mouth.s1': -squeeze / 2,
		'face.y': 0.55 * Math.sin(2 * w + 0.5),
	});
}

export function segments() {
	const out = [];
	const add = (segment) => out.push(segment);

	// -- idle (calm, awake) ----------------------------------------------------
	add({
		name: 'idle',
		kind: 'loop',
		loop: true,
		mood: 'idle',
		timeline: { duration: 2.4, loop: true, keys: [{ at: 0, pose: calm }], procedural: breath(1) },
	});
	for (const gesture of gestureSet('idle', calm)) add(gesture);
	// A greeting for the first moment the face is seen: a slow blink, then a brighter smile.
	add({
		name: 'idle:hello',
		kind: 'gesture',
		mood: 'idle',
		weight: 0,
		timeline: {
			duration: 2.1,
			keys: [
				{ at: 0, pose: calm },
				{ at: 0.1, pose: P(calm, lidsShut('droop', 0.9)), spring: EASE },
				{ at: 0.5, pose: calm, spring: { L: 'soft', R: 'soft', face: 'soft', mouth: 'soft' } },
				{
					at: 0.7,
					pose: P(calm, {
						eyes: shape('round', 0.18, { glint: 0.6, dy: -1 }),
						mouth: { k0: 0.0225, s0: -45, s1: 45 },
						face: { y: -1 },
					}),
					spring: SETTLE,
				},
				{ at: 1.5, pose: calm, spring: SETTLE },
			],
		},
	});

	// -- connecting: the smile becomes a scanner ------------------------------
	add({
		name: 'connecting',
		kind: 'state',
		then: 'connecting-loop',
		timeline: {
			duration: 0.62,
			start: pressed(calm),
			keys: [
				// Anticipation is the press itself (see the "press" segment)...
				{ at: 0.0, pose: pressed(calm), spring: FLINCH },
				// ...then spring up, wide-eyed, with the smile drawn into a dash.
				{
					at: 0.07,
					pose: P(scanBase, { face: { y: -3, sy: 1.012 } }),
					spring: { L: 'pep', R: 'pep', face: 'pop', mouth: 'snap' },
					lag: { R: 0.012 },
				},
				// ...and settle into the exact pose the scanner loop starts from.
				{ at: 0.36, pose: offsetPose(scanBase, scan(0)), spring: SETTLE },
			],
		},
	});
	add({
		name: 'connecting-loop',
		kind: 'loop',
		loop: true,
		timeline: {
			duration: 3 * SCAN_PERIOD,
			loop: true,
			keys: [
				{ at: 0, pose: scanBase },
				// A blink, timed for when the dash turns around.
				{ at: 1.5, pose: P(scanBase, lidsShut('droop')), spring: SHUT },
				{ at: 1.62, pose: scanBase, spring: POP },
			],
			procedural: scan,
		},
	});

	// -- connected: ta-da, then a cool, confident loop -------------------------
	const gather = P(scanBase, {
		eyes: shape('closed', 0.85, { dy: 2.6, sy: 0.95, sx: 1.03 }),
		mouth: { slide: 0, s0: -3, s1: 3, y: 2.4 },
		face: { y: 3.4, sy: 0.975, sx: 1.012 },
	});
	const wow = P(joy, {
		eyes: shape('round', 1, { glint: 1, dy: -3.5, glow: 1.15 }),
		face: { y: -7, sy: 1.035, sx: 0.99 },
	});
	add({
		name: 'connected',
		kind: 'state',
		then: 'connected-loop',
		bridge: 240,
		timeline: {
			duration: 1.95,
			start: scanBase,
			keys: [
				{ at: 0.0, pose: scanBase, spring: SETTLE },
				// Squeeze everything in...
				{ at: 0.03, pose: gather, spring: { L: 'snap', R: 'snap', face: 'crisp', mouth: 'snap' } },
				// ...and let it burst: eyes snap open (passing through the dome), glints
				// flash, the smile unrolls from its dash — right end first.
				{
					at: 0.15,
					pose: wow,
					spring: BOUNCE,
					lag: {
						R: 0.012,
						'L.round': 0.025,
						'R.round': 0.035,
						'L.glint': 0.03,
						'R.glint': 0.04,
						'mouth.s1': 0.01,
						'mouth.s0': 0.04,
					},
				},
				{
					at: 0.4,
					pose: P(joy, { face: { y: -2.4, roll: 2.8 } }),
					spring: { L: 'soft', R: 'soft', face: 'pop', mouth: 'soft' },
				},
				{
					at: 0.58,
					pose: P(joy, { face: { y: -1, roll: -2 } }),
					spring: { L: 'soft', R: 'soft', face: 'pop', mouth: 'soft' },
				},
				{
					at: 0.76,
					pose: P(joy, { face: { y: 0.6, roll: 0.9 } }),
					spring: { L: 'soft', R: 'soft', face: 'soft', mouth: 'soft' },
				},
				// Then cool off into the confident resting look.
				{ at: 1.0, pose: confident, spring: EASE },
			],
		},
	});
	add({
		name: 'connected-loop',
		kind: 'loop',
		loop: true,
		mood: 'connected',
		timeline: {
			duration: 3,
			loop: true,
			keys: [{ at: 0, pose: confident }],
			// The light swells and ebbs with the breath, a touch out of step in each eye.
			procedural: (t, period) => {
				const w = (TAU * t) / period;
				return {
					...breath(0.85)(t, period),
					'L.glow': 0.07 * Math.sin(w),
					'R.glow': 0.07 * (Math.sin(w + 0.6) - Math.sin(0.6)),
				};
			},
		},
	});
	// A relaxed face fidgets less than an idle one.
	const relaxed = new Set(['blink', 'look-left', 'look-right', 'smirk', 'nod']);
	for (const gesture of gestureSet('connected', confident))
		if (relaxed.has(gesture.name.split(':')[1])) add(gesture);
	add({
		name: 'connected:bliss',
		kind: 'gesture',
		mood: 'connected',
		weight: 0.8,
		timeline: {
			duration: 2.3,
			keys: [
				{ at: 0, pose: confident },
				{
					at: 0.05,
					pose: P(confident, {
						eyes: shape('happy', 0.9, { dy: -1 }),
						mouth: { k0: 0.0258, s0: -46, s1: 46, tilt: -0.02, k1: 0 },
						face: { y: -0.8 },
					}),
					spring: EASE,
				},
				{ at: 1.6, pose: confident, spring: { L: 'soft', R: 'soft', face: 'soft', mouth: 'soft' } },
			],
		},
	});

	// -- hover wink (either eye) -------------------------------------------------
	const winkFrom = (rest, left) => {
		const side = (delta) => P(rest, left ? mirror(delta) : delta);
		return [
			// A tiny wind-up, then the wink with a hop and a head tilt.
			{
				at: 0.0,
				pose: side({ face: { roll: -1.4, y: 1.2 }, L: { round: 0.12 }, mouth: { k0: 0.0172 } }),
				spring: SETTLE,
			},
			{
				at: 0.07,
				pose: side({
					L: shape('round', 0.25, { glint: 0.75, dy: -1.2, dx: 1.2 }),
					R: shape('closed', 1, { dy: 1.5, dx: -0.8 }),
					mouth: { k0: 0.0222, k1: 0.0001, tilt: -0.1, s0: -45, s1: 45, x: 1.5 },
					face: { roll: 2.8, y: -2.4, sy: 1.012 },
				}),
				spring: { L: 'pop', R: 'snap', face: 'pop', mouth: 'soft' },
			},
			{ at: 0.62, pose: rest, spring: POP },
		];
	};
	for (const left of [false, true]) {
		const suffix = left ? '-l' : '';
		add({
			name: 'wink' + suffix,
			kind: 'state',
			timeline: { duration: 1.2, keys: winkFrom(calm, left) },
		});
		add({
			name: 'wink-connected' + suffix,
			kind: 'state',
			timeline: { duration: 1.2, start: confident, keys: winkFrom(confident, left) },
		});
	}

	// -- error: a jolt, a shake of the head, then worry ------------------------
	const shake = (at, roll, gaze, y) => ({
		at,
		pose: P(startled, {
			face: { roll, y },
			L: { dx: gaze },
			R: { dx: gaze },
			mouth: { x: -gaze * 0.5 },
		}),
		spring: { L: 'snap', R: 'snap', face: 'snap', mouth: 'crisp' },
	});
	add({
		name: 'error',
		kind: 'state',
		then: 'error-loop',
		// Playback starts already startled; the player's quick bridge into that pose is the jolt.
		bridge: 90,
		timeline: {
			duration: 1.35,
			start: startled,
			keys: [
				{ at: 0, pose: startled, spring: SETTLE },
				shake(0.14, -4.8, -3, -3.5),
				shake(0.25, 4.2, 3, -3),
				shake(0.36, -3.2, -2.4, -2.5),
				shake(0.46, 2.2, 1.6, -1.8),
				shake(0.56, -0.9, -0.6, -1),
				{ at: 0.66, pose: sad, spring: EASE },
			],
		},
	});
	add({
		name: 'error-loop',
		kind: 'loop',
		loop: true,
		timeline: {
			duration: 2.8,
			loop: true,
			keys: [
				{ at: 0, pose: sad },
				{
					at: 1.5,
					pose: P(sad, lidsShut('droop', 0.9)),
					spring: { L: 'crisp', R: 'crisp', face: 'soft', mouth: 'soft' },
				},
				{ at: 1.68, pose: sad, spring: POP },
			],
			procedural: (t, period) => ({
				...breath(0.9)(t, period),
				// A trembling lip: the ripple (part of the sad pose) travels along the mouth.
				'mouth.phase': (TAU * 7 * (t / period)) % TAU,
			}),
		},
	});

	// -- disconnecting: a small "oh", a slump, a nap, a wake-up ---------------
	add({
		name: 'disconnecting',
		kind: 'state',
		then: 'idle',
		timeline: {
			duration: 1.3,
			start: pressed(confident),
			keys: [
				{ at: 0, pose: pressed(confident), spring: FLINCH },
				{
					at: 0.04,
					pose: P(confident, {
						eyes: shape('round', 0.55, { dy: -1.2, glow: 0.5 }),
						mouth: { k0: 0.0176, s0: -37, s1: 37, x: 0, tilt: 0, k1: 0 },
						face: { y: -1.6, roll: 0 },
					}),
					spring: POP,
				},
				{
					at: 0.22,
					pose: P(calm, {
						eyes: shape('droop', 0.72, { dy: 2.2 }),
						mouth: { k0: 0.0125, s0: -38, s1: 38, y: 1.2 },
						face: { y: 2.6, sy: 0.988, sx: 1.004, roll: 0.5 },
					}),
					spring: EASE,
				},
				{
					at: 0.5,
					pose: P(calm, {
						eyes: shape('droop', 1, { dy: 2.6, sy: 0.97 }),
						mouth: { k0: 0.007, s0: -34, s1: 34, y: 2.2 },
						face: { y: 3.2, sy: 0.982 },
					}),
					spring: SLOW,
				},
				{ at: 0.82, pose: calm, spring: POP },
			],
		},
	});

	// -- press: the flinch of being pressed, held until the button is released ----
	for (const [name, rest] of [
		['press', calm],
		['press-connected', confident],
	])
		add({
			name,
			kind: 'state',
			bridge: 100,
			timeline: {
				duration: 0.4,
				start: rest,
				keys: [{ at: 0, pose: pressed(rest), spring: FLINCH }],
			},
		});

	// -- static poses (reduced motion) ----------------------------------------
	for (const [name, pose] of [
		['pose:idle', calm],
		['pose:connected', confident],
		['pose:error', sad],
	])
		add({
			name,
			kind: 'pose',
			timeline: { duration: 1 / 60, start: pose, keys: [{ at: 0, pose }] },
		});

	return out;
}
