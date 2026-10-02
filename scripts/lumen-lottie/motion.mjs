import { CHANNELS, REST } from './geometry.mjs';

export const FPS = 60;
const SUBSTEPS = 24;

/**
 * Spring presets as [angular frequency (rad/s), damping ratio]. Eyes lead and snap,
 * the mouth and the whole head trail them; a ratio below ~0.6 overshoots visibly.
 */
export const SPRINGS = {
	twitch: [90, 0.95],
	snap: [62, 0.78],
	crisp: [46, 0.7],
	pep: [36, 0.55],
	pop: [36, 0.42],
	bounce: [30, 0.3],
	soft: [24, 0.82],
	glide: [17, 1],
	slow: [11, 1],
	drift: [7, 1],
};

export const groupOf = (channel) => channel.split('.')[0];

/** Default timing: eyes react first, then the head, then the mouth. */
export const DEFAULT_LAG = { L: 0, R: 0, face: 0.02, mouth: 0.04 };
const DEFAULT_SPRING = { L: 'crisp', R: 'crisp', face: 'soft', mouth: 'soft' };

const isPlain = (value) => value && typeof value === 'object' && !Array.isArray(value);

/**
 * Deep-merges pose specs (later wins) so poses can be built from reusable parts.
 * `eyes` is sugar for "both L and R"; it is expanded per spec, so a later part can
 * override a single eye without being undone by an earlier symmetrical one.
 */
export function P(...specs) {
	const out = {};
	for (const spec of specs) {
		const { eyes, ...rest } = spec ?? {};
		const expanded = eyes
			? { ...rest, L: { ...eyes, ...(rest.L ?? {}) }, R: { ...eyes, ...(rest.R ?? {}) } }
			: rest;
		for (const [key, value] of Object.entries(expanded))
			out[key] = isPlain(value) ? { ...(out[key] ?? {}), ...value } : value;
	}
	return out;
}

/** Expands a grouped spec — { eyes, L, R, face, mouth } — to a flat channel map. */
export function resolvePose(spec, base = REST) {
	const flat = { ...base };
	const assign = (group, values) => {
		for (const [name, value] of Object.entries(values ?? {})) {
			const channel = `${group}.${name}`;
			if (!(channel in REST)) throw new Error(`Unknown channel ${channel}`);
			flat[channel] = value;
		}
	};
	assign('L', spec.eyes);
	assign('R', spec.eyes);
	assign('L', spec.L);
	assign('R', spec.R);
	assign('face', spec.face);
	assign('mouth', spec.mouth);
	return flat;
}

/**
 * A pose displaced by channel offsets, e.g. the state a procedural loop starts in, so a
 * clip can end exactly where the loop begins.
 */
export function offsetPose(spec, deltas) {
	const flat = resolvePose(spec);
	const moved = {};
	for (const [channel, value] of Object.entries(deltas)) {
		const [group, name] = channel.split('.');
		(moved[group] ??= {})[name] = flat[channel] + value;
	}
	return P(spec, moved);
}

const springOf = (name) => {
	const spring = SPRINGS[name];
	if (!spring) throw new Error(`Unknown spring ${name}`);
	return spring;
};

/**
 * Bakes a storyboard into one channel map per frame.
 *
 *   keys:       [{ at, pose, spring?, lag?, springs? }]  — each pose is complete: channels it
 *               omits head back to rest. `lag` shifts a group's start (seconds);
 *               `spring` is a preset name or { group: name }; `springs` targets single channels.
 *   start:      the pose the clip begins in (default: rest), with zero velocity.
 *   procedural: (t, period) => partial channel map, added on top (periodic for loops).
 */
export function simulate(timeline) {
	const { duration, loop = false, keys, procedural, start } = timeline;
	const frames = Math.round(duration * FPS);
	const periods = loop ? 3 : 1;
	const total = frames * periods;
	const dt = 1 / (FPS * SUBSTEPS);
	const poses = keys.map((key) => resolvePose(key.pose));
	const first = start ? resolvePose(start) : loop ? poses[0] : REST;
	const lagOf = (key, channel) =>
		key.lag?.[channel] ?? key.lag?.[groupOf(channel)] ?? DEFAULT_LAG[groupOf(channel)];
	const springFor = (key, channel) => {
		if (key.springs?.[channel]) return springOf(key.springs[channel]);
		const group = groupOf(channel);
		if (typeof key.spring === 'string') return springOf(key.spring);
		return springOf(key.spring?.[group] ?? DEFAULT_SPRING[group]);
	};
	const out = Array.from({ length: frames + 1 }, () => ({}));
	for (const channel of CHANNELS) {
		const group = groupOf(channel);
		const events = [];
		for (let period = 0; period < periods; period++)
			keys.forEach((key, index) =>
				events.push({
					time: period * duration + key.at + lagOf(key, channel),
					value: poses[index][channel],
					spring: springFor(key, channel),
				}),
			);
		events.sort((a, b) => a.time - b.time);
		let x = first[channel],
			v = 0,
			target = x,
			spring = springOf(DEFAULT_SPRING[group]),
			next = 0;
		const sampleFrom = (periods - 1) * frames;
		for (let step = 0; step <= total * SUBSTEPS; step++) {
			const time = step * dt;
			while (next < events.length && events[next].time <= time + 1e-9) {
				target = events[next].value;
				spring = events[next].spring;
				next++;
			}
			if (step % SUBSTEPS === 0) {
				const frame = step / SUBSTEPS;
				if (frame >= sampleFrom) out[frame - sampleFrom][channel] = x;
			}
			const [omega, zeta] = spring;
			const acceleration = omega * omega * (target - x) - 2 * zeta * omega * v;
			v += acceleration * dt;
			x += v * dt;
		}
	}
	// Procedural motion rides on top of the springs.
	if (procedural)
		out.forEach((frame, index) => {
			const extra = procedural(index / FPS, duration);
			for (const [channel, value] of Object.entries(extra)) {
				if (!(channel in REST)) throw new Error(`Unknown channel ${channel}`);
				frame[channel] += value;
			}
		});
	const report = closeSegment(out, { loop, residualOf: poses.at(-1), procedural, duration });
	return { frames: out, report };
}

/** Forces seamless loop closure / exact end poses and reports what had to be fixed. */
function closeSegment(frames, { loop, residualOf, procedural, duration }) {
	const lastIndex = frames.length - 1;
	const report = { residual: 0, closure: 0 };
	if (loop) {
		const jump = {};
		let worst = 0;
		for (const channel of CHANNELS) {
			jump[channel] = frames[0][channel] - frames[lastIndex][channel];
			worst = Math.max(worst, Math.abs(jump[channel]));
		}
		report.closure = worst;
		// Spread whatever mismatch is left over the final frames so the loop joins exactly.
		const blend = Math.min(24, Math.floor(lastIndex / 3));
		for (let n = 0; n <= blend; n++) {
			const x = 1 - n / blend;
			const weight = x * x * (3 - 2 * x);
			for (const channel of CHANNELS) frames[lastIndex - n][channel] += jump[channel] * weight;
		}
	} else {
		const extra = procedural ? procedural(duration, duration) : {};
		let worst = 0;
		for (const channel of CHANNELS) {
			const finalValue = residualOf[channel] + (extra[channel] ?? 0);
			worst = Math.max(worst, Math.abs(frames[lastIndex][channel] - finalValue));
			frames[lastIndex][channel] = finalValue;
		}
		report.residual = worst;
	}
	return report;
}
